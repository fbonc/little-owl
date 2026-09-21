use iced::widget::{container, markdown, scrollable, text};
use iced::{Element, Fill};

use super::{latex, style};

#[derive(Debug, Clone)]
pub enum Input {
    LinkClicked(markdown::Uri),
}

#[derive(Debug, Default)]
pub(super) struct Answering {
    answer: String,
    markdown: markdown::Content,
    done: bool,
    error: Option<String>,
}

impl Answering {
    pub fn push_token(&mut self, token: &str) {
        self.answer.push_str(token);
        self.markdown = markdown::Content::parse(&latex::normalize_delimiters(&self.answer));
    }

    pub fn finish(&mut self) {
        self.done = true;
    }

    pub fn fail(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn answer(&self) -> &str {
        &self.answer
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn view(&self) -> Element<'_, Input> {
        if let Some(error) = self.error() {
            return text(error)
                .size(style::ERROR_SIZE)
                .color(style::DANGER_COLOR)
                .into();
        }

        let answer = container(markdown::view_with(
            self.markdown.items(),
            markdown::Settings::with_text_size(style::ANSWER_SIZE, style::answer_markdown()),
            &latex::Viewer,
        ))
        .width(Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: style::SCROLL_GUTTER,
            bottom: 0.0,
            left: 0.0,
        });

        scrollable(answer)
            .width(Fill)
            .height(Fill)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(style::SCROLLBAR_WIDTH)
                    .scroller_width(style::SCROLLBAR_WIDTH)
                    .margin(2.0),
            ))
            .style(style::scroll)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_tokens_update_the_source_and_markdown() {
        let mut answering = Answering::default();

        answering.push_token("# Head");
        answering.push_token("ing\n\nBody");

        assert_eq!(answering.answer(), "# Heading\n\nBody");
        assert!(matches!(
            answering.markdown.items().first(),
            Some(markdown::Item::Heading(..))
        ));
        assert!(matches!(
            answering.markdown.items().get(1),
            Some(markdown::Item::Paragraph(..))
        ));
    }

    #[test]
    fn reset_clears_the_previous_response() {
        let mut answering = Answering::default();
        answering.push_token("old answer");
        answering.finish();
        answering.fail("old error".into());

        answering.reset();

        assert!(answering.answer().is_empty());
        assert!(answering.markdown.items().is_empty());
        assert!(!answering.is_done());
        assert!(answering.error().is_none());
    }
}
