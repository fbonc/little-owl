use iced::widget::{Id, button, container, row, space, stack, text, text_input};
use iced::{Center, Element, Fill, Task};

use super::style;

#[derive(Debug)]
pub struct Prompt {
    value: String,
    id: Id,
    focused: bool,
    placeholder: &'static str,
}

#[derive(Debug, Clone)]
pub enum Message {
    Changed(String),
    FocusChanged(bool),
    Submit,
}

impl Prompt {
    pub fn new(placeholder: &'static str) -> Self {
        Self {
            value: String::new(),
            id: Id::unique(),
            focused: false,
            placeholder,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Changed(value) => self.value = value,
            Message::FocusChanged(focused) => self.focused = focused,
            Message::Submit => {}
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn check_focus(&self) -> Task<Message> {
        iced::widget::operation::is_focused(self.id.clone()).map(Message::FocusChanged)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let field = text_input("", &self.value)
            .on_input(Message::Changed)
            .on_submit(Message::Submit)
            .padding(style::INPUT_PADDING)
            .size(style::INPUT_SIZE)
            .style(style::input)
            .id(self.id.clone());

        let placeholder: Element<'_, Message> = if self.value.is_empty() {
            container(
                row![
                    text(self.placeholder)
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
            .on_press(Message::Submit)
            .style(style::send_button);

        container(
            row![stack![field, placeholder], send]
                .spacing(style::HEADER_SPACING)
                .align_y(Center),
        )
        .style(move |theme| style::input_box(theme, self.focused))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_updates_the_local_value() {
        let mut prompt = Prompt::new("Ask the owl…");

        prompt.update(Message::Changed("what is this?".into()));

        assert_eq!(prompt.value(), "what is this?");
    }

    #[test]
    fn submit_does_not_mutate_local_state() {
        let mut prompt = Prompt::new("Ask the owl…");
        prompt.update(Message::Changed("keep me".into()));

        prompt.update(Message::Submit);

        assert_eq!(prompt.value(), "keep me");
    }
}
