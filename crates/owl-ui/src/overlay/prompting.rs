use iced::widget::{Id, button, container, row, space, stack, text, text_input};
use iced::{Center, Element, Fill, Task};

use super::style;

#[derive(Debug)]
pub struct Prompting {
    prompt_value: String,
    input_id: Id,
    input_focused: bool,
    placeholder: &'static str,
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    InputFocusChanged(bool),
    SubmitRequested,
}

impl Prompting {
    pub fn new(placeholder: &'static str) -> Self {
        Self {
            prompt_value: String::new(),
            input_id: Id::unique(),
            input_focused: false,
            placeholder,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::InputChanged(value) => self.prompt_value = value,
            Message::InputFocusChanged(focused) => self.input_focused = focused,
            Message::SubmitRequested => {}
        }
    }

    pub fn prompt_value(&self) -> &str {
        &self.prompt_value
    }

    pub fn check_focus(&self) -> Task<Message> {
        iced::widget::operation::is_focused(self.input_id.clone()).map(Message::InputFocusChanged)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let field = text_input("", &self.prompt_value)
            .on_input(Message::InputChanged)
            .on_submit(Message::SubmitRequested)
            .padding(style::INPUT_PADDING)
            .size(style::INPUT_SIZE)
            .style(style::input)
            .id(self.input_id.clone());

        let placeholder: Element<'_, Message> = if self.prompt_value.is_empty() {
            container(
                row![
                    text(self.placeholder)
                        .size(style::INPUT_SIZE)
                        .color(style::MUTED_COLOR),
                    space().width(Fill),
                    text(style::HINT_TEXT)
                        .size(style::HINT_SIZE)
                        .wrapping(text::Wrapping::None)
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

        let send_icon = container(text("↵").size(style::SEND_SIZE)).center(Fill);

        let send = button(send_icon)
            .on_press(Message::SubmitRequested)
            .width(style::SEND_BUTTON_SIZE)
            .height(style::SEND_BUTTON_SIZE)
            .padding(0)
            .style(style::send_button);

        let send = container(send).padding(style::SEND_BUTTON_INSET);

        container(
            row![stack![field, placeholder], send]
                .spacing(style::INPUT_ACTION_SPACING)
                .align_y(Center),
        )
        .style(move |theme| style::input_box(theme, self.input_focused))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_updates_the_local_value() {
        let mut prompting = Prompting::new("Ask the owl…");

        prompting.update(Message::InputChanged("what is this?".into()));

        assert_eq!(prompting.prompt_value(), "what is this?");
    }

    #[test]
    fn submit_does_not_mutate_local_state() {
        let mut prompting = Prompting::new("Ask the owl…");
        prompting.update(Message::InputChanged("keep me".into()));

        prompting.update(Message::SubmitRequested);

        assert_eq!(prompting.prompt_value(), "keep me");
    }
}
