use std::sync::LazyLock;

use iced::widget::{
    button, column, container, image, row, scrollable, space, stack, text, text_input
};
use iced::widget::Id;
use iced::{Center, Color, Element, Fill, Task};

pub use owl_types::{Target, UiEvent};

static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../assets/logo.png").as_slice())
});

#[derive(Debug, Clone)]
pub enum ViewMessage {
    InputChanged(String),
    InputFocusChanged(bool),
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

#[derive(Debug)]
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>,
    pub input: String,
    placeholder: &'static str,
    pub phase: Phase,
    pub answer: String,
    pub done: bool,
    pub error: Option<String>,

    input_id: Id,
    input_focused: bool,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            input: String::new(),
            placeholder: random_placeholder(),
            phase: Phase::default(),
            answer: String::new(),
            done: false,
            error: None,
            input_id: Id::unique(),
            input_focused: false,
        }
    }
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

    // Apply a widget interaction. Returns a `Submit` for the host to route to core when the user commits (Enter).
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
            ViewMessage::InputFocusChanged(focused) => {
                self.input_focused = focused;
                None
            }
        }
    }

    fn commit(&self) -> Submit {
        let ask = (!self.input.is_empty()).then(|| self.input.clone());
        Submit { ask }
    }

    pub fn check_input_focus(&self) -> Task<ViewMessage> {
        iced::widget::operation::is_focused(self.input_id.clone())
        .map(ViewMessage::InputFocusChanged)
    }
}

// Pick a placeholder at random. RandomState is OS-seeded,
// so an empty hasher's finish() is a cheap random value with no rng dependency.
fn random_placeholder() -> &'static str {
    use std::hash::{BuildHasher, Hasher};
    let n = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish() as usize;
    style::PLACEHOLDERS[n % style::PLACEHOLDERS.len()]
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
    pub const SCROLLER_COLOR: Color = Color {
        r: 0.72,
        g: 0.60,
        b: 0.45,
        a: 0.55,
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
    pub const LOGO_SIZE: f32 = 40.0;
    pub const HEADER_SPACING: f32 = 10.0;
    pub const SCROLLBAR_WIDTH: f32 = 6.0;
    pub const SCROLL_GUTTER: f32 = 14.0;

    pub const TARGET_SIZE: f32 = 16.0;
    pub const INPUT_SIZE: f32 = 16.0;
    pub const ANSWER_SIZE: f32 = 15.0;
    pub const ERROR_SIZE: f32 = 14.0;
    pub const SEND_SIZE: f32 = 24.0;

    pub const PLACEHOLDERS: [&str; 7] = [
        "Hoot away…",
        "Hoot’s on your mind?",
        "Hoot me a question…",
        "Ask the owl…",
        "I’m owl ears…",
        "Whooo’s curious?",
        "Perch a thought…"
    ];
    pub const IMAGE_TARGET_LABEL: &str = "[image capture]";

    pub const HINT_TEXT: &str = "Or just press Enter — I’ll try to figure out what you need";
    pub const HINT_SIZE: f32 = 12.5;
    pub const HINT_SPACING: f32 = 8.0;
    // Dimmer than MUTED so the hint reads as secondary next to the phrase.
    pub const HINT_COLOR: Color = Color {
        r: 0.56,
        g: 0.47,
        b: 0.39,
        a: 1.0,
    };
}

pub fn view(overlay: &Overlay) -> Element<'_, ViewMessage> {
    let mut card = column![].spacing(style::CARD_SPACING);

    let mut header = row![].spacing(style::HEADER_SPACING).align_y(Center);

    header = header.push(
        image(LOGO.clone())
            .width(style::LOGO_SIZE)
            .height(style::LOGO_SIZE),
    );

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

    card = card.push(header);

    match overlay.phase {
        Phase::Asking => {
            let field = text_input("", &overlay.input)
                .on_input(ViewMessage::InputChanged)
                .on_submit(ViewMessage::Submit)
                .padding(style::INPUT_PADDING)
                .size(style::INPUT_SIZE)
                .style(input_style)
                .id(overlay.input_id.clone());

            let placeholder: Element<'_, ViewMessage> = if overlay.input.is_empty() {
                container(
                    row![
                        text(overlay.placeholder)
                            .size(style::INPUT_SIZE)
                            .color(style::MUTED_COLOR),
                        space().width(Fill),
                        text(style::HINT_TEXT)
                            .size(style::HINT_SIZE)
                            .color(style::HINT_COLOR),
                    ]
                    .spacing(style::HINT_SPACING)
                    .align_y(Center),
                )
                .padding(style::INPUT_PADDING)
                .into()
            } else {
                text("").into()
            };

            let send = button(text(" ↵ ").size(style::SEND_SIZE))
                .on_press(ViewMessage::Submit)
                .style(send_button_style);

            let input_focused = overlay.input_focused;

            let input_box: Element<'_, ViewMessage> = container(
                row![stack![field, placeholder], send]
                .spacing(style::HEADER_SPACING)
                .align_y(Center),
            )
            .style(move |theme| input_box_style(theme, input_focused))
            .into();

            card = card.push(
                input_box
            );
        }
        Phase::Answering => {
            if let Some(error) = &overlay.error {
                card = card.push(
                    text(error.clone())
                        .size(style::ERROR_SIZE)
                        .color(style::DANGER_COLOR),
                );
            } else {
                let answer = container(
                    text(overlay.answer.clone())
                        .size(style::ANSWER_SIZE)
                        .color(style::TEXT_COLOR),
                )
                .padding(iced::Padding {
                    top: 0.0,
                    right: style::SCROLL_GUTTER,
                    bottom: 0.0,
                    left: 0.0,
                });

                card = card.push(
                    scrollable(answer)
                        .direction(scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new()
                                .width(style::SCROLLBAR_WIDTH)
                                .scroller_width(style::SCROLLBAR_WIDTH)
                                .margin(2.0),
                        ))
                        .style(scroll_style),
                );
            }
        }
    }

    container(card)
        .padding(style::CARD_PADDING)
        .width(Fill)
        .height(Fill)
        .style(card_style)
        .into()
}

fn input_box_style(_theme: &iced::Theme, focused: bool) -> container::Style {
    let border_color = if focused {
        style::ACCENT_COLOR
    } else {
        style::INPUT_BORDER_COLOR
    };

    container::Style {
        background: Some(style::INPUT_BACKGROUND.into()),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: style::INPUT_RADIUS.into(),
        },
        text_color: Some(Color::TRANSPARENT),
        shadow: iced::Shadow::default(),
        snap: false
    }
}

fn input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: style::MUTED_COLOR,
        placeholder: style::MUTED_COLOR,
        value: style::TEXT_COLOR,
        selection: style::SELECTION_COLOR,
    }
}

fn send_button_style(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let background = match status {
        Status::Hovered => iced::Color {
            r: 0.98,
            g: 0.78,
            b: 0.42,
            a: 1.0,
        },
        Status::Pressed => iced::Color {
            r: 0.84,
            g: 0.62,
            b: 0.24,
            a: 1.0,
        },
        _ => style::ACCENT_COLOR,
    };
    iced::widget::button::Style {
        background: Some(background.into()),
        text_color: style::BACKGROUND_COLOR,
        border: iced::Border {
            radius: style::INPUT_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
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

fn scroll_style(theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let mut base = scrollable::default(theme, status);
    base.vertical_rail.background = None;
    base.vertical_rail.border = iced::Border::default();
    base.vertical_rail.scroller.background = style::SCROLLER_COLOR.into();
    base
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
