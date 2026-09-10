use std::sync::LazyLock;

use iced::widget::{column, container, image, row, scrollable, text, text_input};
use iced::{Center, Element, Fill};

pub use owl_types::{Target, UiEvent};

// Built once: `image::Handle::from_bytes` mints a fresh id each call, so building
// it per frame would defeat iced's decoded-image cache.
static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../assets/logo.png").as_slice())
});

#[derive(Debug, Clone)]
pub enum ViewMessage {
    InputChanged(String),
    Submit,
}

#[derive(Debug, Clone)]
pub struct Submit {
    pub ask: Option<String>,
}

/// Which half of the flow the overlay is in. Enter moves Asking -> Answering:
/// the prompt disappears and the answer streams in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Asking,
    Answering,
}

#[derive(Debug, Default)]
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>, // The captured target, or None until capture completes.
    pub input: String,          // The prompt text. Empty means the default action.
    pub phase: Phase,
    pub answer: String,
    pub done: bool,
    pub error: Option<String>,
}

impl Overlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, event: UiEvent) {
        match event {
            UiEvent::Show => self.visible = true,
            UiEvent::Target(target) => self.target = Some(target),
            UiEvent::Token(token) => self.answer.push_str(&token),
            UiEvent::Done => self.done = true,
            UiEvent::Error(error) => self.error = Some(error),
        }
    }

    /// Apply a widget interaction. Returns a `Submit` for the host to route to core
    /// when the user commits (Enter).
    pub fn on_view_message(&mut self, message: ViewMessage) -> Option<Submit> {
        match message {
            ViewMessage::InputChanged(value) => {
                self.input = value;
                None
            }
            ViewMessage::Submit => {
                self.phase = Phase::Answering;
                Some(self.commit())
            }
        }
    }

    fn commit(&self) -> Submit {
        let ask = (!self.input.is_empty()).then(|| self.input.clone());
        Submit { ask }
    }
}

mod style {
    use iced::border::Radius;
    use iced::{Border, Color};

    // Palette pulled from the owl logo: cocoa ground, cream text, warm tan, amber.
    pub const TEXT_COLOR: Color = Color {
        r: 0.95,
        g: 0.88,
        b: 0.77,
        a: 1.0,
    };
    pub const MUTED_COLOR: Color = Color {
        r: 0.72,
        g: 0.60,
        b: 0.45,
        a: 1.0,
    };
    pub const DANGER_COLOR: Color = Color {
        r: 0.90,
        g: 0.44,
        b: 0.38,
        a: 1.0,
    };
    pub const ACCENT_COLOR: Color = Color {
        r: 0.93,
        g: 0.70,
        b: 0.29,
        a: 1.0,
    };

    pub const BACKGROUND_COLOR: Color = Color {
        r: 0.17,
        g: 0.095,
        b: 0.078,
        a: 1.0,
    };
    pub const INPUT_BACKGROUND: Color = Color {
        r: 0.24,
        g: 0.15,
        b: 0.12,
        a: 1.0,
    };
    pub const INPUT_BORDER_COLOR: Color = Color {
        r: 0.72,
        g: 0.60,
        b: 0.45,
        a: 0.30,
    };
    pub const SELECTION_COLOR: Color = Color {
        r: 0.93,
        g: 0.70,
        b: 0.29,
        a: 0.35,
    };
    pub const BORDER_COLOR: Color = Color {
        r: 0.93,
        g: 0.70,
        b: 0.29,
        a: 0.22,
    };
    pub const BORDER_STYLE: Border = Border {
        color: BORDER_COLOR,
        width: 1.0,
        radius: Radius {
            top_left: 16.0,
            top_right: 16.0,
            bottom_right: 16.0,
            bottom_left: 16.0,
        },
    };

    pub const CARD_SPACING: f32 = 14.0;
    pub const CARD_PADDING: f32 = 22.0;
    pub const INPUT_PADDING: f32 = 12.0;
    pub const INPUT_RADIUS: f32 = 10.0;
    pub const LOGO_SIZE: f32 = 30.0;
    pub const HEADER_SPACING: f32 = 10.0;

    pub const TARGET_SIZE: f32 = 13.0;
    pub const INPUT_SIZE: f32 = 16.0;
    pub const ANSWER_SIZE: f32 = 15.0;
    pub const ERROR_SIZE: f32 = 14.0;

    pub const PROMPT_PLACEHOLDER: &str = "Hoot me...  (Enter to explain or define)";
    pub const IMAGE_TARGET_LABEL: &str = "[image capture]";
}

pub fn view(overlay: &Overlay) -> Element<'_, ViewMessage> {
    let mut ask_card = column![].spacing(style::CARD_SPACING);

    let mut header = row![
        image(LOGO.clone())
            .width(style::LOGO_SIZE)
            .height(style::LOGO_SIZE)
    ]
    .spacing(style::HEADER_SPACING)
    .align_y(Center);
    if let Some(target) = &overlay.target {
        let label = match target {
            Target::Text(t) => t.text.clone(),
            Target::Image(_) => style::IMAGE_TARGET_LABEL.to_string(),
        };
        header = header.push(
            text(label)
                .size(style::TARGET_SIZE)
                .color(style::MUTED_COLOR),
        );
    }
    ask_card = ask_card.push(header);

    match overlay.phase {
        Phase::Asking => {
            ask_card = ask_card.push(
                text_input(style::PROMPT_PLACEHOLDER, &overlay.input)
                    .on_input(ViewMessage::InputChanged)
                    .on_submit(ViewMessage::Submit)
                    .padding(style::INPUT_PADDING)
                    .size(style::INPUT_SIZE)
                    .style(input_style),
            );
        }
        Phase::Answering => {
            if let Some(error) = &overlay.error {
                ask_card = ask_card.push(
                    text(error.clone())
                        .size(style::ERROR_SIZE)
                        .color(style::DANGER_COLOR),
                );
            } else {
                ask_card = ask_card.push(scrollable(
                    text(overlay.answer.clone())
                        .size(style::ANSWER_SIZE)
                        .color(style::TEXT_COLOR),
                ));
            }
        }
    }

    // Fill the window so the opaque card covers it edge to edge; a smaller card in
    // a transparent window would show the desktop through the uncovered margin.
    container(ask_card)
        .padding(style::CARD_PADDING)
        .width(Fill)
        .height(Fill)
        .style(card_style)
        .into()
}

fn input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => style::ACCENT_COLOR,
        _ => style::INPUT_BORDER_COLOR,
    };
    text_input::Style {
        background: style::INPUT_BACKGROUND.into(),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: style::INPUT_RADIUS.into(),
        },
        icon: style::MUTED_COLOR,
        placeholder: style::MUTED_COLOR,
        value: style::TEXT_COLOR,
        selection: style::SELECTION_COLOR,
    }
}

fn card_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(style::BACKGROUND_COLOR.into()),
        border: style::BORDER_STYLE,
        text_color: Some(style::TEXT_COLOR),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owl_types::{TextCapture, TextCaptureMethod};

    #[test]
    fn empty_input_is_the_default_action() {
        let mut overlay = Overlay::new();
        overlay.apply(UiEvent::Show);
        overlay.apply(UiEvent::Target(Target::Text(TextCapture {
            text: "epistemic uncertainty".into(),
            method: TextCaptureMethod::Accessibility,
        })));
        let submit = overlay.commit();
        assert!(overlay.visible);
        assert!(submit.ask.is_none());
    }

    #[test]
    fn typed_text_becomes_the_ask() {
        let mut overlay = Overlay::new();
        overlay.input = "in one sentence".into();
        assert_eq!(overlay.commit().ask.as_deref(), Some("in one sentence"));
    }

    #[test]
    fn submit_moves_from_asking_to_answering() {
        let mut overlay = Overlay::new();
        assert_eq!(overlay.phase, Phase::Asking);
        assert!(overlay.on_view_message(ViewMessage::Submit).is_some());
        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn tokens_accumulate_into_the_answer() {
        let mut overlay = Overlay::new();
        overlay.apply(UiEvent::Token("un".into()));
        overlay.apply(UiEvent::Token("certainty".into()));
        overlay.apply(UiEvent::Done);
        assert_eq!(overlay.answer, "uncertainty");
        assert!(overlay.done);
    }
}
