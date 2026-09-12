use iced::widget::mouse_area;
use iced::{Element, Subscription, Task, Theme, window};
use owl_core::{Event as CoreEvent, SubmitSender};
use owl_ui::overlay;
use owl_ui::{CoreMessage, Overlay, OverlayOutput};

const WINDOW_WIDTH: f32 = 500.0;
const MIN_WINDOW_WIDTH: f32 = 225.0;
const PROMPTING_HEIGHT: f32 = 150.0;
const MIN_WINDOW_HEIGHT: f32 = PROMPTING_HEIGHT;
const ANSWERING_HEIGHT: f32 = 360.0;

struct App {
    overlay: Overlay,
    to_core: Option<SubmitSender>,
    window: Option<window::Id>,
}

#[derive(Debug, Clone)]
enum Message {
    WindowOpened(Option<window::Id>),
    DragWindow,
    Core(CoreEvent),
    Overlay(overlay::Message),
    CheckPromptInputFocus,
}

fn main() -> iced::Result {
    iced::application(
        || {
            (
                App {
                    overlay: Overlay::new(),
                    to_core: None,
                    window: None,
                },
                window::latest().map(Message::WindowOpened),
            )
        },
        update,
        view,
    )
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

fn update(state: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::WindowOpened(id) => {
            state.window = id;
            Task::none()
        }
        Message::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Message::Core(CoreEvent::Ready(sender)) => {
            state.to_core = Some(sender);
            Task::none()
        }
        Message::Core(CoreEvent::Message(core_message)) => {
            let should_show = matches!(core_message, CoreMessage::Show);
            state.overlay.apply(core_message);

            match (should_show, state.window) {
                (true, Some(id)) => {
                    window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                }
                _ => Task::none(),
            }
        }
        Message::Overlay(message) => match state.overlay.update(message) {
            Some(OverlayOutput::Submitted(submit)) => {
                if let Some(sender) = &mut state.to_core {
                    let _ = sender.try_send(submit);
                }
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
        Message::CheckPromptInputFocus => {
            if state.overlay.visible && state.overlay.phase == owl_ui::Phase::Prompting {
                state
                    .overlay
                    .check_prompt_input_focus()
                    .map(Message::Overlay)
            } else {
                Task::none()
            }
        }
    }
}

fn view(state: &App) -> Element<'_, Message> {
    mouse_area(owl_ui::view(&state.overlay).map(Message::Overlay))
        .on_press(Message::DragWindow)
        .into()
}

fn subscription(_state: &App) -> Subscription<Message> {
    Subscription::batch([
        Subscription::run(owl_core::run).map(Message::Core),
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
                Some(Message::CheckPromptInputFocus)
            }
            iced::Event::Window(window::Event::CloseRequested) => {
                Some(Message::Overlay(overlay::Message::DismissRequested))
            }
            _ => None,
        }),
    ])
}
