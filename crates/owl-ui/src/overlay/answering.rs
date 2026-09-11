use iced::Element;
use iced::widget::{container, scrollable, text};

use super::style;

pub(super) fn view<'a, Message: 'a>(
    answer: &'a str,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    if let Some(error) = error {
        return text(error)
            .size(style::ERROR_SIZE)
            .color(style::DANGER_COLOR)
            .into();
    }

    let answer = container(
        text(answer)
            .size(style::ANSWER_SIZE)
            .color(style::TEXT_COLOR),
    )
    .width(iced::Fill)
    .padding(iced::Padding {
        top: 0.0,
        right: style::SCROLL_GUTTER,
        bottom: 0.0,
        left: 0.0,
    });

    scrollable(answer)
        .width(iced::Fill)
        .height(iced::Fill)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(style::SCROLLBAR_WIDTH)
                .scroller_width(style::SCROLLBAR_WIDTH)
                .margin(2.0),
        ))
        .style(style::scroll)
        .into()
}
