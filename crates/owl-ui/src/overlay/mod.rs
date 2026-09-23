use std::sync::LazyLock;

use iced::widget::{button, column, container, image, row, space, text};
use iced::{Center, Element, Fill, Task};
use iced::advanced::text::{Wrapping};

use owl_types::Target;

pub mod answering;
mod latex;
pub mod prompting;
mod style;

static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../../../assets/logo.png").as_slice())
});

#[derive(Debug, Clone)]
pub enum Input {
    Prompting(prompting::Input),
    Answering(answering::Input),
    Show,
    SetTarget(Target),
    AppendAnswer(String),
    FinishAnswer,
    FailAnswer(String),
    BackRequested,
    DismissRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Submitted { prompt: Option<String> },
    LinkClicked(String),
    PhaseChanged(Phase),
    Dismissed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Prompting,
    Answering,
}

#[derive(Debug)]
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>,
    prompting: prompting::Prompting,
    answering: answering::Answering,
    pub phase: Phase,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            prompting: prompting::Prompting::new(random_placeholder()),
            answering: answering::Answering::default(),
            phase: Phase::default(),
        }
    }
}

impl Overlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, input: Input) -> Option<Output> {
        match input {
            Input::Prompting(prompting::Input::SubmitRequested) => {
                self.phase = Phase::Answering;
                self.answering.reset();
                Some(Output::Submitted {
                    prompt: self.commit(),
                })
            }
            Input::Prompting(input) => {
                self.prompting.update(input);
                None
            }
            Input::Answering(answering::Input::LinkClicked(uri)) => Some(Output::LinkClicked(uri)),
            Input::Show => {
                self.visible = true;
                None
            }
            Input::SetTarget(target) => {
                self.target = Some(target);
                None
            }
            Input::AppendAnswer(chunk) => {
                self.answering.push_token(&chunk);
                None
            }
            Input::FinishAnswer => {
                self.answering.finish();
                None
            }
            Input::FailAnswer(error) => {
                self.answering.fail(error);
                None
            }
            Input::BackRequested => {
                self.phase = Phase::Prompting;
                Some(Output::PhaseChanged(Phase::Prompting))
            }
            Input::DismissRequested => {
                self.visible = false;
                Some(Output::Dismissed)
            }
        }
    }

    pub fn check_prompt_input_focus(&self) -> Task<Input> {
        self.prompting.check_focus().map(Input::Prompting)
    }

    pub fn answer(&self) -> &str {
        self.answering.answer()
    }

    pub fn answer_done(&self) -> bool {
        self.answering.is_done()
    }

    pub fn answer_error(&self) -> Option<&str> {
        self.answering.error()
    }

    pub fn view(&self) -> Element<'_, Input> {
        let content = match self.phase {
            Phase::Prompting => self.prompting.view().map(Input::Prompting),
            Phase::Answering => self.answering.view().map(Input::Answering),
        };

        container(
            column![header(self), content]
                .spacing(style::CARD_SPACING)
                .width(Fill)
                .height(Fill),
        )
        .padding(style::CARD_PADDING)
        .width(Fill)
        .height(Fill)
        .style(style::card)
        .into()
    }

    fn commit(&self) -> Option<String> {
        (!self.prompting.prompt_value().is_empty())
            .then(|| self.prompting.prompt_value().to_owned())
    }
}

fn header(overlay: &Overlay) -> Element<'_, Input> {
    let mut header = row![
        image(LOGO.clone())
            .width(style::LOGO_SIZE)
            .height(style::LOGO_SIZE)
    ]
    .spacing(style::HEADER_SPACING)
    .align_y(Center);

    if let Some(target) = &overlay.target {
        let label = match target {
            Target::Text(text) => text.text.as_str().replace("\n", "").replace("\r", ""),
            Target::Image(_) => style::IMAGE_TARGET_LABEL.to_string(),
        };

        header = header.push(
            text(label)
                .size(style::TARGET_SIZE)
                .color(style::MUTED_COLOR)
                .wrapping(Wrapping::None),
        );
    }

    let mut actions = row![].spacing(style::HEADER_ACTION_SPACING);

    if overlay.phase == Phase::Answering {
        actions = actions.push(header_button("←", Input::BackRequested));
    }

    actions = actions.push(header_button("×", Input::DismissRequested));

    header = header.push(space().width(Fill)).push(actions);

    header.into()
}

fn header_button(label: &'static str, input: Input) -> Element<'static, Input> {
    let icon = container(text(label).size(style::HEADER_ACTION_ICON_SIZE)).center(Fill);

    button(icon)
        .on_press(input)
        .width(style::HEADER_ACTION_SIZE)
        .height(style::HEADER_ACTION_SIZE)
        .padding(0)
        .style(style::header_button)
        .into()
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
        overlay.update(Input::Show);
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "epistemic uncertainty".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted { prompt: None })
        );
        assert!(overlay.visible);
    }

    #[test]
    fn typed_text_becomes_the_ask() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "in one sentence".into(),
        )));

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: Some("in one sentence".into()),
            })
        );
    }

    #[test]
    fn submit_moves_from_prompting_to_answering() {
        let mut overlay = Overlay::new();
        assert_eq!(overlay.phase, Phase::Prompting);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_some()
        );

        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn prompting_messages_stay_inside_the_overlay() {
        let mut overlay = Overlay::new();

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::InputChanged(
                    "why?".into()
                )))
                .is_none()
        );
        assert_eq!(overlay.phase, Phase::Prompting);
    }

    #[test]
    fn markdown_link_clicks_bubble_up_to_the_host() {
        let mut overlay = Overlay::new();

        assert_eq!(
            overlay.update(Input::Answering(answering::Input::LinkClicked(
                "https://example.com".into()
            ))),
            Some(Output::LinkClicked("https://example.com".into()))
        );
    }

    #[test]
    fn tokens_accumulate_into_the_answer() {
        let mut overlay = Overlay::new();
        overlay.update(Input::AppendAnswer("un".into()));
        overlay.update(Input::AppendAnswer("certainty".into()));
        overlay.update(Input::FinishAnswer);

        assert_eq!(overlay.answer(), "uncertainty");
        assert!(overlay.answer_done());
    }

    #[test]
    fn back_returns_to_prompting() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;

        assert_eq!(
            overlay.update(Input::BackRequested),
            Some(Output::PhaseChanged(Phase::Prompting))
        );
        assert_eq!(overlay.phase, Phase::Prompting);
    }

    #[test]
    fn close_hides_the_overlay() {
        let mut overlay = Overlay::new();
        overlay.visible = true;

        assert_eq!(
            overlay.update(Input::DismissRequested),
            Some(Output::Dismissed)
        );
        assert!(!overlay.visible);
    }

    #[test]
    fn submitting_clears_the_previous_response() {
        let mut overlay = Overlay::new();
        overlay.answering.push_token("old answer");
        overlay.answering.finish();
        overlay.answering.fail("old error".into());

        overlay.update(Input::Prompting(prompting::Input::SubmitRequested));

        assert!(overlay.answer().is_empty());
        assert!(!overlay.answer_done());
        assert!(overlay.answer_error().is_none());
    }
}
