use std::sync::LazyLock;

use iced::advanced::text::Wrapping;
use iced::widget::{button, column, container, image, row, space, svg, text};
use iced::{Center, Element, Fill, Subscription, Task};

use owl_provider::ModelSelection;
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
    CaptureCompleted,
    SetTarget(Target),
    CaptureFailed,
    AppendAnswer(String),
    FinishAnswer,
    FailAnswer(String),
    RemoveTargetRequested,
    BackRequested,
    DismissRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Submitted {
        prompt: Option<String>,
        model: Option<ModelSelection>,
    },
    TargetRemoved,
    CaptureRegionRequested,
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
    selected_model: Option<ModelSelection>,
    prompting: prompting::Prompting,
    answering: answering::Answering,
    capture_ready: bool,
    pub phase: Phase,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            visible: false,
            target: None,
            selected_model: None,
            prompting: prompting::Prompting::new(random_placeholder()),
            answering: answering::Answering::default(),
            capture_ready: true,
            phase: Phase::default(),
        }
    }
}

impl Overlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_selected_model(mut self, selected_model: Option<ModelSelection>) -> Self {
        self.selected_model = selected_model;
        self
    }

    pub fn update(&mut self, input: Input) -> Option<Output> {
        match input {
            Input::Prompting(prompting::Input::SubmitRequested) if self.capture_ready => {
                self.phase = Phase::Answering;
                self.answering.reset();
                Some(Output::Submitted {
                    prompt: self.commit(),
                    model: self.selected_model.clone(),
                })
            }
            Input::Prompting(prompting::Input::CaptureRegionRequested) => {
                self.prompting
                    .update(prompting::Input::CaptureRegionRequested);
                Some(Output::CaptureRegionRequested)
            }
            Input::Prompting(prompting::Input::ModelSelected(selection)) => {
                self.selected_model = Some(selection);
                None
            }
            Input::Prompting(input) => {
                self.prompting.update(input);
                None
            }
            Input::Answering(answering::Input::LinkClicked(uri)) => Some(Output::LinkClicked(uri)),
            Input::Answering(input) => {
                self.answering.update(input);
                None
            }
            Input::Show => {
                self.visible = true;
                self.target = None;
                self.capture_ready = false;
                self.phase = Phase::Prompting;
                self.prompting.reset();
                None
            }
            Input::CaptureCompleted => {
                self.capture_ready = true;
                None
            }
            Input::SetTarget(target) => {
                self.target = Some(target);
                None
            }
            Input::RemoveTargetRequested if self.phase == Phase::Prompting => {
                self.target.take().map(|_| Output::TargetRemoved)
            }
            Input::RemoveTargetRequested => None,
            Input::CaptureFailed => {
                self.prompting.update(prompting::Input::CaptureFailed);
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

    pub fn subscription(&self) -> Subscription<Input> {
        if self.phase == Phase::Answering {
            self.answering.subscription().map(Input::Answering)
        } else {
            Subscription::none()
        }
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

    pub fn view(&self, available_models: Vec<ModelSelection>) -> Element<'_, Input> {
        let selected_model = self
            .selected_model
            .clone()
            .filter(|selected| available_models.contains(selected));
        let content = match self.phase {
            Phase::Prompting => self
                .prompting
                .view(available_models, selected_model, self.capture_ready)
                .map(Input::Prompting),
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
    let target: Element<'_, Input> = if let Some(target) = &overlay.target {
        let label = match target {
            Target::Text(text) => text
                .text
                .as_str()
                .trim()
                .replace("\n", "")
                .replace("\r", "")
                .replace("\t", ""),
            Target::Image(_) => style::IMAGE_TARGET_LABEL.to_string(),
        };

        let mut target = row![
            container(
                text(label)
                    .size(style::TARGET_SIZE)
                    .color(style::MUTED_COLOR)
                    .width(Fill)
                    .wrapping(Wrapping::None),
            )
            .width(Fill)
            .clip(true)
        ];

        if overlay.phase == Phase::Prompting {
            target = target.push(target_clear_button());
        }

        target
            .spacing(style::HEADER_ACTION_SPACING)
            .align_y(Center)
            .width(Fill)
            .into()
    } else {
        space().width(Fill).into()
    };

    let mut actions = row![].spacing(style::HEADER_ACTION_SPACING);

    if overlay.phase == Phase::Answering {
        actions = actions.push(header_button("←", Input::BackRequested));
    }

    actions = actions.push(header_button("×", Input::DismissRequested));

    row![
        image(LOGO.clone())
            .width(style::LOGO_SIZE)
            .height(style::LOGO_SIZE),
        target,
        actions,
    ]
    .spacing(style::HEADER_SPACING)
    .align_y(Center)
    .into()
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

fn target_clear_button() -> Element<'static, Input> {
    let icon = svg(svg::Handle::from_memory(
        include_bytes!("../../../../assets/clear-target.svg").as_slice(),
    ))
    .width(style::TARGET_CLEAR_ICON_SIZE)
    .height(style::TARGET_CLEAR_ICON_SIZE);

    button(container(icon).center(Fill))
        .on_press(Input::RemoveTargetRequested)
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
    use owl_provider::ProviderId;
    use owl_types::{TextCapture, TextCaptureMethod};

    #[test]
    fn empty_prompt_is_the_default_action() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Show);
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "epistemic uncertainty".into(),
            method: TextCaptureMethod::Accessibility,
        })));
        overlay.update(Input::CaptureCompleted);

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: None,
                model: None,
            })
        );
        assert!(overlay.visible);
    }

    #[test]
    fn showing_again_clears_the_previous_target() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "previous selection".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        overlay.update(Input::Show);

        assert!(overlay.target.is_none());
        assert!(overlay.visible);
    }

    #[test]
    fn submission_waits_for_capture_completion() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Show);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_none()
        );
        assert_eq!(overlay.phase, Phase::Prompting);

        overlay.update(Input::CaptureCompleted);

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::SubmitRequested))
                .is_some()
        );
        assert_eq!(overlay.phase, Phase::Answering);
    }

    #[test]
    fn removing_target_clears_it_and_notifies_the_host() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "selected text".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        assert_eq!(
            overlay.update(Input::RemoveTargetRequested),
            Some(Output::TargetRemoved)
        );
        assert!(overlay.target.is_none());
        assert!(overlay.update(Input::RemoveTargetRequested).is_none());
    }

    #[test]
    fn target_cannot_be_removed_while_answering() {
        let mut overlay = Overlay::new();
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "selected text".into(),
            method: TextCaptureMethod::Accessibility,
        })));
        overlay.phase = Phase::Answering;

        assert!(overlay.update(Input::RemoveTargetRequested).is_none());
        assert!(overlay.target.is_some());
    }

    #[test]
    fn showing_again_returns_to_prompting_with_the_new_target() {
        let mut overlay = Overlay::new();
        overlay.phase = Phase::Answering;
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "old prompt".into(),
        )));
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "previous selection".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        overlay.update(Input::Show);
        overlay.update(Input::SetTarget(Target::Text(TextCapture {
            text: "new selection".into(),
            method: TextCaptureMethod::Accessibility,
        })));

        assert_eq!(overlay.phase, Phase::Prompting);
        assert!(overlay.prompting.prompt_value().is_empty());
        assert!(matches!(
            overlay.target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "new selection"
        ));
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
                model: None,
            })
        );
    }

    #[test]
    fn model_selection_is_stored_and_included_in_submission() {
        let mut overlay = Overlay::new();
        let selection = ModelSelection::new(ProviderId::new("openai"), "gpt-test");

        assert!(
            overlay
                .update(Input::Prompting(prompting::Input::ModelSelected(
                    selection.clone()
                )))
                .is_none()
        );
        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::SubmitRequested)),
            Some(Output::Submitted {
                prompt: None,
                model: Some(selection),
            })
        );
    }

    #[test]
    fn region_capture_request_keeps_the_prompt() {
        let mut overlay = Overlay::new();
        overlay.update(Input::Prompting(prompting::Input::InputChanged(
            "what is shown?".into(),
        )));

        assert_eq!(
            overlay.update(Input::Prompting(prompting::Input::CaptureRegionRequested)),
            Some(Output::CaptureRegionRequested)
        );
        assert_eq!(overlay.phase, Phase::Prompting);
        assert_eq!(overlay.prompting.prompt_value(), "what is shown?");
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
