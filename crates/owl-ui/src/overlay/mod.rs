use std::sync::LazyLock;

use iced::widget::{column, container, image, row, text};
use iced::{Center, Element, Fill, Task};

use owl_types::{Target, UiEvent};

mod answer;
pub mod prompt;
mod style;

static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../../assets/logo.png").as_slice())
});

#[derive(Debug, Clone)]
pub enum Message {
    Prompt(prompt::Message),
    Submit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Submitted(Submit),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submit {
    pub prompt: Option<String>,
}

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
    prompt: prompt::Prompt,
    pub phase: Phase,
    pub answer: String,
    pub done: bool,
    pub error: Option<String>,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            prompt: prompt::Prompt::new(random_placeholder()),
            phase: Phase::default(),
            answer: String::new(),
            done: false,
            error: None,
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

    pub fn update(&mut self, message: Message) -> Option<Event> {
        match message {
            Message::Prompt(message) => {
                self.prompt.update(message);
                None
            }
            Message::Submit => {
                self.phase = Phase::Answering;
                Some(Event::Submitted(self.commit()))
            }
        }
    }

    pub fn check_prompt_focus(&self) -> Task<Message> {
        self.prompt.check_focus().map(Message::Prompt)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = match self.phase {
            Phase::Asking => self.prompt.view().map(|message| match message {
                prompt::Message::Submit => Message::Submit,
                message => Message::Prompt(message),
            }),
            Phase::Answering => answer::view(&self.answer, self.error.as_deref()),
        };

        container(column![header(self), content].spacing(style::CARD_SPACING))
            .padding(style::CARD_PADDING)
            .width(Fill)
            .height(Fill)
            .style(style::card)
            .into()
    }

    fn commit(&self) -> Submit {
        let prompt = (!self.prompt.value().is_empty()).then(|| self.prompt.value().to_owned());
        Submit { prompt: prompt }
    }
}

fn header(overlay: &Overlay) -> Element<'_, Message> {
    let mut header = row![
        image(LOGO.clone())
            .width(style::LOGO_SIZE)
            .height(style::LOGO_SIZE)
    ]
    .spacing(style::HEADER_SPACING)
    .align_y(Center);

    if let Some(target) = &overlay.target {
        let label = match target {
            Target::Text(text) => text.text.as_str(),
            Target::Image(_) => style::IMAGE_TARGET_LABEL,
        };
        header = header.push(
            text(label)
                .size(style::TARGET_SIZE)
                .color(style::MUTED_COLOR),
        );
    }

    header.into()
}

fn random_placeholder() -> &'static str {
    use std::hash::{BuildHasher, Hasher};

    let n = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish() as usize;
    style::PLACEHOLDERS[n % style::PLACEHOLDERS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use owl_types::{TextCapture, TextCaptureMethod};

    #[test]
    fn empty_prompt_is_the_default_action() {
        let mut overlay = Overlay::new();
        overlay.apply(UiEvent::Show);
        overlay.apply(UiEvent::Target(Target::Text(TextCapture {
            text: "epistemic uncertainty".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        assert_eq!(
            overlay.update(Message::Submit),
            Some(Event::Submitted(Submit { prompt: None }))
        );
        assert!(overlay.visible);
    }

    #[test]
    fn typed_text_becomes_the_ask() {
        let mut overlay = Overlay::new();
        overlay.update(Message::Prompt(prompt::Message::Changed(
            "in one sentence".into(),
        )));

        assert_eq!(
            overlay.update(Message::Submit),
            Some(Event::Submitted(Submit {
                prompt: Some("in one sentence".into()),
            }))
        );
    }

    #[test]
    fn submit_moves_from_asking_to_answering() {
        let mut overlay = Overlay::new();
        assert_eq!(overlay.phase, Phase::Asking);

        assert!(overlay.update(Message::Submit).is_some());

        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn prompt_messages_stay_inside_the_overlay() {
        let mut overlay = Overlay::new();

        assert!(
            overlay
                .update(Message::Prompt(prompt::Message::Changed("why?".into())))
                .is_none()
        );
        assert_eq!(overlay.phase, Phase::Asking);
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
