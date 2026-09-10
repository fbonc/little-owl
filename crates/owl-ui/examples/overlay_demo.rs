//! Throwaway host for the overlay: opens a window and drives owl-ui with scripted
//! mock events (Show -> Target -> streaming Tokens -> Done) plus a live prompt.
//! Delete this once owl-app hosts `owl_ui::view` / `Overlay` for real.
//!
//!   cargo run -p owl-ui --example overlay_demo

use std::collections::VecDeque;
use std::time::Duration;

use iced::time::every;
use iced::{Element, Subscription, Task, Theme, window};
use owl_types::{TextCapture, TextCaptureMethod};

use owl_ui::{Overlay, Target, UiEvent, ViewMessage};

struct Demo {
    overlay: Overlay,
    core: MockCore,
}

struct MockCore {
    queue: VecDeque<UiEvent>,
}

impl MockCore {
    /// The opening state: show the overlay with a captured target, awaiting the
    /// ask. The answer only streams once the user hits Enter (see `replay`).
    fn lookup() -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(UiEvent::Show);
        queue.push_back(UiEvent::Target(Target::Text(TextCapture {
            text: "epistemic uncertainty".into(),
            method: TextCaptureMethod::Accessibility,
        })));
        Self { queue }
    }

    /// Pull the next scripted event, if any.
    fn pop(&mut self) -> Option<UiEvent> {
        self.queue.pop_front()
    }

    /// Queue a streamed answer for the submitted ask (empty = the default explain).
    fn replay(&mut self, ask: Option<&str>) {
        let body = match ask {
            Some(ask) => format!("You asked: {ask}. Here is a mocked streamed reply."),
            None => "Epistemic uncertainty is uncertainty that comes from a lack of \
                     knowledge, reducible in principle with more data or a better model."
                .to_string(),
        };
        self.queue = body
            .split_inclusive(' ')
            .map(|token| UiEvent::Token(token.to_string()))
            .collect();
        self.queue.push_back(UiEvent::Done);
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    View(ViewMessage),
}

fn main() -> iced::Result {
    iced::application(
        || Demo {
            overlay: Overlay::new(),
            core: MockCore::lookup(),
        },
        update,
        view,
    )
    .subscription(subscription)
    .theme(|_state: &Demo| Theme::Dark)
    .title("little owl")
    .window(window::Settings {
        size: iced::Size::new(640.0, 420.0),
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
        Message::Tick => {
            if let Some(event) = state.core.pop() {
                state.overlay.apply(event);
            }
        }
        Message::View(view_message) => {
            // In owl-app this Submit goes to core; here MockCore just replays a
            // canned answer for whatever was asked.
            if let Some(submit) = state.overlay.on_view_message(view_message) {
                state.overlay.answer.clear();
                state.overlay.done = false;
                state.core.replay(submit.ask.as_deref());
            }
        }
    }
    Task::none()
}

fn view(state: &Demo) -> Element<'_, Message> {
    owl_ui::view(&state.overlay).map(Message::View)
}

fn subscription(_state: &Demo) -> Subscription<Message> {
    every(Duration::from_millis(110)).map(|_| Message::Tick)
}
