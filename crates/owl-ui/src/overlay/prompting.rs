use iced::widget::{Id, button, column, container, row, space, stack, svg, text, text_input};
use iced::{Center, Element, Fill, Task};

use super::style;

#[derive(Debug)]
pub struct Prompting {
    prompt_value: String,
    input_id: Id,
    input_focused: bool,
    capture_failed: bool,
    placeholder: &'static str,
}

#[derive(Debug, Clone)]
pub enum Input {
    InputChanged(String),
    InputFocusChanged(bool),
    SubmitRequested,
    CaptureRegionRequested,
    CaptureFailed,
}

impl Prompting {
    pub fn new(placeholder: &'static str) -> Self {
        Self {
            prompt_value: String::new(),
            input_id: Id::unique(),
            input_focused: false,
            capture_failed: false,
            placeholder,
        }
    }

    pub fn update(&mut self, input: Input) {
        match input {
            Input::InputChanged(value) => self.prompt_value = value,
            Input::InputFocusChanged(focused) => self.input_focused = focused,
            Input::CaptureRegionRequested => self.capture_failed = false,
            Input::CaptureFailed => self.capture_failed = true,
            Input::SubmitRequested => {}
        }
    }

    pub fn prompt_value(&self) -> &str {
        &self.prompt_value
    }

    pub fn clear_capture_failure(&mut self) {
        self.capture_failed = false;
    }

    pub fn check_focus(&self) -> Task<Input> {
        iced::widget::operation::is_focused(self.input_id.clone()).map(Input::InputFocusChanged)
    }

    pub fn view(&self) -> Element<'_, Input> {
        let field = text_input("", &self.prompt_value)
            .on_input(Input::InputChanged)
            .on_submit(Input::SubmitRequested)
            .padding(style::INPUT_PADDING)
            .size(style::INPUT_SIZE)
            .style(style::input)
            .id(self.input_id.clone());

        let placeholder: Element<'_, Input> = if self.prompt_value.is_empty() {
            let hint = container(row![
                space().width(Fill),
                text(style::HINT_TEXT)
                    .size(style::HINT_SIZE)
                    .wrapping(text::Wrapping::None)
                    .color(style::HINT_COLOR),
            ])
            .width(Fill)
            .clip(true);

            container(
                row![
                    text(self.placeholder)
                        .size(style::INPUT_SIZE)
                        .color(style::MUTED_COLOR),
                    hint,
                ]
                .spacing(style::HINT_SPACING)
                .align_y(Center),
            )
            .padding(style::INPUT_PADDING)
            .into()
        } else {
            text("").into()
        };

        let send_icon = container(text("↵").size(style::SEND_SIZE)).center(Fill);

        let send = button(send_icon)
            .on_press(Input::SubmitRequested)
            .width(style::SEND_BUTTON_SIZE)
            .height(style::SEND_BUTTON_SIZE)
            .padding(0)
            .style(style::send_button);

        let send = container(send).padding(style::SEND_BUTTON_INSET);

        let prompt_bar = container(
            row![stack![field, placeholder], send]
                .spacing(style::INPUT_ACTION_SPACING)
                .align_y(Center),
        )
        .width(Fill)
        .style(move |theme| style::input_box(theme, self.input_focused));

        let icon = svg(svg::Handle::from_memory(
            include_bytes!("../../../../assets/capture-region.svg").as_slice(),
        ))
        .width(18)
        .height(18);
        let capture = button(
            row![
                icon,
                text("Capture Screen").size(style::CAPTURE_BUTTON_TEXT_SIZE)
            ]
            .spacing(7)
            .align_y(Center),
        )
        .on_press(Input::CaptureRegionRequested)
        .padding([5, 8])
        .style(style::capture_button);

        let mut capture_row = row![capture].spacing(8).align_y(Center);
        if self.capture_failed {
            capture_row = capture_row.push(
                text("Capture failed. Try again.")
                    .size(style::CAPTURE_BUTTON_TEXT_SIZE)
                    .color(style::DANGER_COLOR),
            );
        }

        column![prompt_bar, capture_row]
            .spacing(style::CAPTURE_BUTTON_SPACING)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_updates_the_local_value() {
        let mut prompting = Prompting::new("Ask the owl…");

        prompting.update(Input::InputChanged("what is this?".into()));

        assert_eq!(prompting.prompt_value(), "what is this?");
    }

    #[test]
    fn submit_does_not_mutate_local_state() {
        let mut prompting = Prompting::new("Ask the owl…");
        prompting.update(Input::InputChanged("keep me".into()));

        prompting.update(Input::SubmitRequested);

        assert_eq!(prompting.prompt_value(), "keep me");
    }
}
