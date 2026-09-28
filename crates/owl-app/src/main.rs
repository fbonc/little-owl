use std::sync::Arc;

use iced::widget::mouse_area;
use iced::{Element, Subscription, Task, Theme, window};
use owl_config::{AppConfig, CredentialStore, KeychainCredentialStore};
use owl_core::{Output as CoreOutput, Sender as CoreSender};
use owl_provider::{
    ModelSelection, OPENAI_PROVIDER_ID, OpenAiConfig, OpenAiProvider, ProviderId, ProviderRegistry,
};
use owl_types::WindowBounds;
use owl_ui::overlay;
use owl_ui::{Overlay, OverlayOutput, Target};

const WINDOW_WIDTH: f32 = 500.0;
const MIN_WINDOW_WIDTH: f32 = 225.0;
const PROMPTING_HEIGHT: f32 = 180.0;
const MIN_WINDOW_HEIGHT: f32 = PROMPTING_HEIGHT;
const ANSWERING_HEIGHT: f32 = 360.0;

struct App {
    overlay: Overlay,
    providers: ProviderRegistry,
    to_core: CoreSender,
    window: Option<window::Id>,
    selecting_region: bool,
}

#[derive(Debug, Clone)]
enum Input {
    WindowOpened(Option<window::Id>),
    DragWindow,
    Core(CoreOutput),
    Overlay(overlay::Input),
    CheckPromptInputFocus,
    BeginRegionSelection,
}

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .subscription(subscription)
        .theme(|_state: &App| Theme::Dark)
        .title("little owl")
        .window(window::Settings {
            size: iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
            min_size: Some(iced::Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
            position: window::Position::Centered,
            visible: false,
            decorations: false,
            transparent: true,
            resizable: true,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            ..Default::default()
        })
        .style(|_state: &App, _theme: &iced::Theme| iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::BLACK,
        })
        .run()
}

fn boot() -> (App, Task<Input>) {
    let (providers, selected_model) = load_providers();
    let (to_core, core_outputs) = owl_core::start(providers.clone());

    (
        App {
            overlay: Overlay::new().with_selected_model(selected_model),
            providers,
            to_core,
            window: None,
            selecting_region: false,
        },
        Task::batch([
            window::latest().map(Input::WindowOpened),
            Task::run(core_outputs, Input::Core),
        ]),
    )
}

fn load_providers() -> (ProviderRegistry, Option<ModelSelection>) {
    let config = AppConfig::load().unwrap_or_else(|error| {
        eprintln!("failed to load application configuration: {error}");
        AppConfig::default()
    });
    let providers = providers_from_config(&config, &KeychainCredentialStore::default());
    let available_models = providers.available_models();
    let selected_model = config
        .selected_model
        .filter(|model| available_models.contains(model))
        .or_else(|| available_models.into_iter().next());

    (providers, selected_model)
}

fn providers_from_config(
    config: &AppConfig,
    credentials: &dyn CredentialStore,
) -> ProviderRegistry {
    let providers = ProviderRegistry::new();
    let provider_id = ProviderId::new(OPENAI_PROVIDER_ID);
    let Some(settings) = config
        .providers
        .get(&provider_id)
        .filter(|settings| settings.enabled)
    else {
        return providers;
    };

    match credentials.get(&provider_id) {
        Ok(Some(api_key)) => {
            providers.add_provider(
                provider_id,
                Arc::new(OpenAiProvider::new(OpenAiConfig::new(
                    api_key,
                    settings.models.clone(),
                ))),
            );
        }
        Ok(None) => eprintln!("OpenAI is enabled but has no stored API key"),
        Err(error) => eprintln!("failed to load the OpenAI API key: {error}"),
    }

    providers
}

fn update(state: &mut App, input: Input) -> Task<Input> {
    match input {
        Input::WindowOpened(id) => {
            state.window = id;
            match id {
                Some(id) => configure_window_for_active_space(id),
                None => Task::none(),
            }
        }
        Input::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Input::Core(output) => match output {
            CoreOutput::ShowRequested { focused_window } => {
                let _ = state.overlay.update(overlay::Input::Show);
                match state.window {
                    Some(id) => show_overlay(id, focused_window),
                    None => Task::none(),
                }
            }
            CoreOutput::CaptureCompleted => {
                let _ = state.overlay.update(overlay::Input::CaptureCompleted);
                Task::none()
            }
            CoreOutput::TargetCaptured(target) => {
                let _ = state.overlay.update(overlay::Input::SetTarget(target));
                Task::none()
            }
            CoreOutput::RegionSelectionFinished(result) => {
                state.selecting_region = false;
                match result {
                    Ok(Some(image)) => {
                        let _ = state
                            .overlay
                            .update(overlay::Input::SetTarget(Target::Image(image)));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("region selection failed: {error}");
                        let _ = state.overlay.update(overlay::Input::CaptureFailed);
                    }
                }
                match state.window {
                    Some(id) => {
                        window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                    }
                    None => Task::none(),
                }
            }
            CoreOutput::AnswerChunk(chunk) => {
                let _ = state.overlay.update(overlay::Input::AppendAnswer(chunk));
                Task::none()
            }
            CoreOutput::AnswerCompleted => {
                let _ = state.overlay.update(overlay::Input::FinishAnswer);
                Task::none()
            }
            CoreOutput::RequestFailed(error) => {
                eprintln!("core request failed: {error}");
                let _ = state.overlay.update(overlay::Input::FailAnswer(error));
                Task::none()
            }
        },
        Input::Overlay(input) => match state.overlay.update(input) {
            Some(OverlayOutput::Submitted { prompt, model }) => {
                let available_models = state.providers.available_models();
                let Some(model) = model.filter(|selected| available_models.contains(selected))
                else {
                    let _ = state.overlay.update(overlay::Input::FailAnswer(
                        "no available model selected".into(),
                    ));
                    return Task::none();
                };
                let _ = state
                    .to_core
                    .try_send(owl_core::Input::Submit { prompt, model });
                match state.window {
                    Some(id) => window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT)),
                    None => Task::none(),
                }
            }
            Some(OverlayOutput::TargetRemoved) => {
                if let Err(error) = state.to_core.try_send(owl_core::Input::RemoveTarget) {
                    eprintln!("failed to remove target from core: {error}");
                }
                Task::none()
            }
            Some(OverlayOutput::CaptureRegionRequested) => {
                if state.selecting_region {
                    return Task::none();
                }
                let Some(id) = state.window else {
                    return Task::none();
                };
                state.selecting_region = true;
                window::set_mode(id, window::Mode::Hidden)
                    .chain(Task::done(Input::BeginRegionSelection))
            }
            Some(OverlayOutput::LinkClicked(uri)) => {
                println!("link clicked: {uri}");
                Task::none()
            }
            Some(OverlayOutput::PhaseChanged(phase)) => match (phase, state.window) {
                (owl_ui::Phase::Prompting, Some(id)) => {
                    window::resize(id, iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT))
                }
                (owl_ui::Phase::Answering, Some(id)) => {
                    window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT))
                }
                (_, None) => Task::none(),
            },
            Some(OverlayOutput::Dismissed) => match state.window {
                Some(id) => window::set_mode(id, window::Mode::Hidden),
                None => Task::none(),
            },
            None => Task::none(),
        },
        Input::CheckPromptInputFocus => {
            if state.overlay.visible && state.overlay.phase == owl_ui::Phase::Prompting {
                state.overlay.check_prompt_input_focus().map(Input::Overlay)
            } else {
                Task::none()
            }
        }
        Input::BeginRegionSelection => {
            if let Err(error) = state.to_core.try_send(owl_core::Input::SelectRegion) {
                state.selecting_region = false;
                eprintln!("region selection failed to start: {error}");
                let _ = state.overlay.update(overlay::Input::CaptureFailed);
                match state.window {
                    Some(id) => {
                        window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                    }
                    None => Task::none(),
                }
            } else {
                Task::none()
            }
        }
    }
}

fn view(state: &App) -> Element<'_, Input> {
    let available_models = state.providers.available_models();

    mouse_area(owl_ui::view(&state.overlay, available_models).map(Input::Overlay))
        .on_press(Input::DragWindow)
        .into()
}

fn subscription(state: &App) -> Subscription<Input> {
    Subscription::batch([
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
                Some(Input::CheckPromptInputFocus)
            }
            iced::Event::Window(window::Event::CloseRequested) => {
                Some(Input::Overlay(overlay::Input::DismissRequested))
            }
            _ => None,
        }),
        state.overlay.subscription().map(Input::Overlay),
    ])
}

fn show_overlay(id: window::Id, focused_window: Option<WindowBounds>) -> Task<Input> {
    let task = window::set_mode(id, window::Mode::Hidden).chain(window::resize(
        id,
        iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
    ));
    let task = match focused_window {
        Some(bounds) => task.chain(window::move_to(id, overlay_position(bounds))),
        None => task,
    };

    task.chain(window::set_mode(id, window::Mode::Windowed))
        .chain(window::gain_focus(id))
}

fn overlay_position(bounds: WindowBounds) -> iced::Point {
    iced::Point::new(
        (bounds.x + (bounds.w - f64::from(WINDOW_WIDTH)) / 2.0) as f32,
        (bounds.y + (bounds.h - f64::from(PROMPTING_HEIGHT)) / 2.0) as f32,
    )
}

#[cfg(target_os = "macos")]
fn configure_window_for_active_space(id: window::Id) -> Task<Input> {
    window::run(id, |window| {
        use iced::window::raw_window_handle::RawWindowHandle;
        use objc2::rc::Retained;
        use objc2_app_kit::{NSView, NSWindowCollectionBehavior};

        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // The AppKit raw handle owns a valid NSView for the duration of this callback.
        let Some(view): Option<Retained<NSView>> =
            (unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) })
        else {
            return;
        };
        let Some(window) = view.window() else {
            return;
        };
        window.setCollectionBehavior(
            window.collectionBehavior() | NSWindowCollectionBehavior::MoveToActiveSpace,
        );
    })
    .discard()
}

#[cfg(not(target_os = "macos"))]
fn configure_window_for_active_space(_id: window::Id) -> Task<Input> {
    Task::none()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use owl_config::{ProviderSettings, Result};

    use super::*;

    struct TestCredentials(Option<String>);

    impl CredentialStore for TestCredentials {
        fn get(&self, _provider: &ProviderId) -> Result<Option<String>> {
            Ok(self.0.clone())
        }

        fn set(&self, _provider: &ProviderId, _api_key: &str) -> Result<()> {
            Ok(())
        }

        fn remove(&self, _provider: &ProviderId) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn overlay_is_centered_on_the_focused_window() {
        assert_eq!(
            overlay_position(WindowBounds {
                x: 100.0,
                y: 200.0,
                w: 1_200.0,
                h: 800.0,
            }),
            iced::Point::new(450.0, 510.0)
        );
    }

    #[test]
    fn registers_enabled_openai_provider_with_stored_credentials() {
        let provider_id = ProviderId::new(OPENAI_PROVIDER_ID);
        let config = AppConfig {
            providers: BTreeMap::from([(
                provider_id.clone(),
                ProviderSettings {
                    enabled: true,
                    models: vec!["gpt-test".into()],
                },
            )]),
            ..AppConfig::default()
        };

        let providers = providers_from_config(&config, &TestCredentials(Some("test-key".into())));

        assert_eq!(
            providers.available_models(),
            vec![ModelSelection::new(provider_id, "gpt-test")]
        );
    }

    #[test]
    fn skips_openai_provider_without_stored_credentials() {
        let config = AppConfig {
            providers: BTreeMap::from([(
                ProviderId::new(OPENAI_PROVIDER_ID),
                ProviderSettings {
                    enabled: true,
                    models: vec!["gpt-test".into()],
                },
            )]),
            ..AppConfig::default()
        };

        let providers = providers_from_config(&config, &TestCredentials(None));

        assert!(providers.available_models().is_empty());
    }
}
