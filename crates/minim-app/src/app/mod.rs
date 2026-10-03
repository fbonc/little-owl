use iced::{Task, Theme, window as iced_window};
use minim_core::{Output as CoreOutput, Sender as CoreSender};
use minim_provider::ProviderRegistry;
use minim_ui::Overlay;
use minim_ui::overlay;

mod providers;
mod update;
mod view;
mod window;

const WINDOW_WIDTH: f32 = 500.0;
const MIN_WINDOW_WIDTH: f32 = 225.0;
const PROMPTING_HEIGHT: f32 = 180.0;
const MIN_WINDOW_HEIGHT: f32 = PROMPTING_HEIGHT;
const ANSWERING_HEIGHT: f32 = 360.0;

struct App {
    overlay: Overlay,
    providers: ProviderRegistry,
    to_core: CoreSender,
    window: Option<iced_window::Id>,
    selecting_region: bool,
}

#[derive(Debug, Clone)]
enum Input {
    WindowOpened(Option<iced_window::Id>),
    DragWindow,
    Core(CoreOutput),
    Overlay(overlay::Input),
    CheckPromptInputFocus,
    BeginRegionSelection,
}

pub(super) fn run() -> iced::Result {
    iced::application(boot, update::update, view::view)
        .subscription(view::subscription)
        .theme(|_state: &App| Theme::Dark)
        .title("minim")
        .window(iced_window::Settings {
            size: iced::Size::new(WINDOW_WIDTH, PROMPTING_HEIGHT),
            min_size: Some(iced::Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
            position: iced_window::Position::Centered,
            visible: false,
            decorations: false,
            transparent: true,
            resizable: true,
            level: iced_window::Level::AlwaysOnTop,
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
    let (providers, selected_model) = providers::load_providers();
    let (to_core, core_outputs) = minim_core::start(providers.clone());

    (
        App {
            overlay: Overlay::new().with_selected_model(selected_model),
            providers,
            to_core,
            window: None,
            selecting_region: false,
        },
        Task::batch([
            iced_window::latest().map(Input::WindowOpened),
            Task::run(core_outputs, Input::Core),
        ]),
    )
}
