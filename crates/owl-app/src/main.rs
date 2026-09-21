use iced::widget::mouse_area;
use iced::{Element, Subscription, Task, Theme, window};
use owl_core::{Output as CoreOutput, Sender as CoreSender};
use owl_ui::overlay;
use owl_ui::{Overlay, OverlayOutput};

const WINDOW_WIDTH: f32 = 500.0;
const MIN_WINDOW_WIDTH: f32 = 225.0;
const PROMPTING_HEIGHT: f32 = 150.0;
const MIN_WINDOW_HEIGHT: f32 = PROMPTING_HEIGHT;
const ANSWERING_HEIGHT: f32 = 360.0;

struct App {
    overlay: Overlay,
    to_core: CoreSender,
    window: Option<window::Id>,
}

#[derive(Debug, Clone)]
enum Input {
    WindowOpened(Option<window::Id>),
    DragWindow,
    Core(CoreOutput),
    Overlay(overlay::Input),
    CheckPromptInputFocus,
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
    let (to_core, core_outputs) = owl_core::start();

    (
        App {
            overlay: Overlay::new(),
            to_core,
            window: None,
        },
        Task::batch([
            window::latest().map(Input::WindowOpened),
            Task::run(core_outputs, Input::Core),
        ]),
    )
}

fn update(state: &mut App, input: Input) -> Task<Input> {
    match input {
        Input::WindowOpened(id) => {
            state.window = id;
            Task::none()
        }
        Input::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Input::Core(output) => match output {
            CoreOutput::ShowRequested => {
                let _ = state.overlay.update(overlay::Input::Show);
                match state.window {
                    Some(id) => {
                        window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                    }
                    None => Task::none(),
                }
            }
            CoreOutput::TargetCaptured(target) => {
                let _ = state.overlay.update(overlay::Input::SetTarget(target));
                Task::none()
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
                let _ = state.overlay.update(overlay::Input::FailAnswer(error));
                Task::none()
            }
        },
        Input::Overlay(input) => match state.overlay.update(input) {
            Some(OverlayOutput::Submitted { prompt }) => {
                let _ = state.to_core.try_send(owl_core::Input::Submit { prompt });
                match state.window {
                    Some(id) => window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWERING_HEIGHT)),
                    None => Task::none(),
                }
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
    }
}

fn view(state: &App) -> Element<'_, Input> {
    mouse_area(owl_ui::view(&state.overlay).map(Input::Overlay))
        .on_press(Input::DragWindow)
        .into()
}

fn subscription(_state: &App) -> Subscription<Input> {
    iced::event::listen_with(|event, _status, _window| match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
        | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
            Some(Input::CheckPromptInputFocus)
        }
        iced::Event::Window(window::Event::CloseRequested) => {
            Some(Input::Overlay(overlay::Input::DismissRequested))
        }
        _ => None,
    })
}
