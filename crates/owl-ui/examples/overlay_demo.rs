//! Throwaway host for the overlay, wired like the real app will be. Two channels
//! bridge the mock backend and the UI, exactly as owl-core will: core -> ui carries
//! UiEvents (a stream the subscription drains and folds via `apply`), ui -> core
//! carries the submitted ask. Delete this once owl-app hosts owl_ui for real.
//!
//!   cargo run -p owl-ui --example overlay_demo

use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::mouse_area;
use iced::{Element, Subscription, Task, Theme, window};
use tokio::time::sleep;

use owl_types::{TextCapture, TextCaptureMethod};
use owl_ui::{Overlay, Phase, Submit, Target, UiEvent, ViewMessage};

const WINDOW_WIDTH: f32 = 640.0;
const ASKING_HEIGHT: f32 = 128.0;
const MAX_HEIGHT: f32 = 440.0;

struct Demo {
    overlay: Overlay,
    // Send half of the ui -> core channel; None until the backend hands it over.
    to_core: Option<mpsc::Sender<Submit>>,
    window: Option<window::Id>,
}

#[derive(Debug, Clone)]
enum Message {
    WindowOpened(Option<window::Id>),
    DragWindow,
    Ready(mpsc::Sender<Submit>),
    Core(UiEvent),
    View(ViewMessage),
}

fn main() -> iced::Result {
    iced::application(
        || {
            (
                Demo {
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
    .theme(|_state: &Demo| Theme::Dark)
    .title("little owl")
    .window(window::Settings {
        size: iced::Size::new(WINDOW_WIDTH, ASKING_HEIGHT),
        position: window::Position::Centered,
        decorations: false,
        transparent: true,
        resizable: true,
        ..Default::default()
    })
    .style(|_state: &Demo, _theme: &iced::Theme| iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: iced::Color::BLACK,
    })
    .run()
}

fn update(state: &mut Demo, message: Message) -> Task<Message> {
    match message {
        Message::WindowOpened(id) => {
            state.window = id;
            fit_window(state)
        }
        Message::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Message::Ready(sender) => {
            state.to_core = Some(sender);
            Task::none()
        }
        Message::Core(event) => {
            state.overlay.apply(event);
            fit_window(state)
        }
        Message::View(view_message) => {
            if let Some(submit) = state.overlay.on_view_message(view_message)
                && let Some(sender) = &mut state.to_core
            {
                let _ = sender.try_send(submit);
            }
            fit_window(state)
        }
    }
}

fn view(state: &Demo) -> Element<'_, Message> {
    // The whole panel is a drag handle (borderless: no title bar to grab).
    mouse_area(owl_ui::view(&state.overlay).map(Message::View))
        .on_press(Message::DragWindow)
        .into()
}

/// Resize the window to fit the overlay's current content.
fn fit_window(state: &Demo) -> Task<Message> {
    match state.window {
        Some(id) => window::resize(id, content_size(&state.overlay)),
        None => Task::none(),
    }
}

/// Estimate the height the overlay wants. Iced can't measure laid-out content, so
/// this is approximate (text metrics are guessed); err generous to avoid clipping,
/// and the scrollable inside handles anything past MAX_HEIGHT.
fn content_size(overlay: &Overlay) -> iced::Size {
    let height = match overlay.phase {
        Phase::Asking => ASKING_HEIGHT,
        Phase::Answering => {
            const CHROME: f32 = 44.0 + 17.0 + 14.0; // padding*2 + target line + spacing
            const LINE: f32 = 21.0;
            let per_line = ((WINDOW_WIDTH - 44.0) / 7.6).max(1.0);
            let lines = (overlay.answer.chars().count() as f32 / per_line)
                .ceil()
                .max(1.0);
            (CHROME + lines * LINE).clamp(ASKING_HEIGHT, MAX_HEIGHT)
        }
    };
    iced::Size::new(WINDOW_WIDTH, height)
}

fn subscription(_state: &Demo) -> Subscription<Message> {
    Subscription::run(mock_core)
}

/// Stand-in for owl-core. Opens the ui -> core channel and hands the UI its sender,
/// streams the opening Show + Target, then streams an answer for each ask it
/// receives. Same shape as the real core; only the contents are canned.
fn mock_core() -> impl Stream<Item = Message> {
    iced::stream::channel(16, |mut to_ui: mpsc::Sender<Message>| async move {
        let (to_core, mut asks) = mpsc::channel::<Submit>(1);
        let _ = to_ui.send(Message::Ready(to_core)).await;

        let _ = to_ui.send(Message::Core(UiEvent::Show)).await;
        let _ = to_ui
            .send(Message::Core(UiEvent::Target(sample_target())))
            .await;

        while let Some(submit) = asks.next().await {
            for event in answer_events(submit.ask.as_deref()) {
                let _ = to_ui.send(Message::Core(event)).await;
                sleep(Duration::from_millis(55)).await;
            }
        }
    })
}

fn sample_target() -> Target {
    Target::Text(TextCapture {
        text: "epistemic uncertainty".into(),
        method: TextCaptureMethod::Accessibility,
    })
}

fn answer_events(ask: Option<&str>) -> Vec<UiEvent> {
    let body = match ask {
        Some(ask) => format!("You asked: {ask}. Here is a mocked streamed reply."),
        None => "Epistemic uncertainty is uncertainty that comes from a lack of \
                 knowledge, reducible in principle with more data or a better model."
            .to_string(),
    };
    let mut events: Vec<UiEvent> = body
        .split_inclusive(' ')
        .map(|token| UiEvent::Token(token.to_string()))
        .collect();
    events.push(UiEvent::Done);
    events
}
