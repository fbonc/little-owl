use std::borrow::Cow;

use iced::widget::{container, markdown, rich_text, row, svg, text};
use iced::{Center, Element, Fill, Length, Pixels};

use super::answering::Input;
use super::style;

const ESCAPED_DOLLAR: char = '\u{e000}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineFragment<'a> {
    Text(&'a str),
    Math(&'a str),
}

pub(super) fn normalize_delimiters(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut line_start = true;
    let mut inline_code = None;
    let mut fence = None;

    while index < bytes.len() {
        if line_start && let Some((delimiter, length)) = fence_at(bytes, index) {
            match fence {
                Some((open_delimiter, open_length))
                    if delimiter == open_delimiter && length >= open_length =>
                {
                    fence = None;
                }
                None => fence = Some((delimiter, length)),
                _ => {}
            }

            let line_end = source[index..]
                .find('\n')
                .map_or(source.len(), |offset| index + offset + 1);
            normalized.push_str(&source[index..line_end]);
            line_start = line_end < source.len() || source.ends_with('\n');
            index = line_end;
            continue;
        }

        let byte = bytes[index];

        if fence.is_none() && byte == b'`' {
            let length = delimiter_run(bytes, index, b'`');

            match inline_code {
                Some(open_length) if length == open_length => inline_code = None,
                None => inline_code = Some(length),
                _ => {}
            }

            normalized.push_str(&source[index..index + length]);
            index += length;
            line_start = false;
            continue;
        }

        if fence.is_none() && inline_code.is_none() && byte == b'\\' {
            let length = delimiter_run(bytes, index, b'\\');
            let next = bytes.get(index + length).copied();

            if length % 2 == 1 {
                let replacement = match next {
                    Some(b'(' | b')') => Some("$"),
                    Some(b'[' | b']') => Some("$$"),
                    Some(b'$') => Some("\u{e000}"),
                    _ => None,
                };

                if let Some(replacement) = replacement {
                    normalized.push_str(&source[index..index + length - 1]);
                    normalized.push_str(replacement);
                    index += length + 1;
                    line_start = false;
                    continue;
                }
            }

            normalized.push_str(&source[index..index + length]);
            index += length;
            line_start = false;
            continue;
        }

        let character = source[index..].chars().next().expect("character boundary");
        normalized.push(character);
        index += character.len_utf8();
        line_start = character == '\n';
    }

    fence_display_math(&normalized)
}

fn fence_display_math(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut pending = None::<String>;
    let mut body = String::new();
    let mut fence = None;

    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = content.trim();

        if let Some(original) = &mut pending {
            if trimmed == "$$" {
                let source = body.trim();
                if source.is_empty() {
                    original.push_str(line);
                    output.push_str(original);
                } else {
                    output.push_str("```math\n");
                    output.push_str(source);
                    output.push_str("\n```");
                    if line.ends_with('\n') {
                        output.push('\n');
                    }
                }
                pending = None;
                body.clear();
            } else {
                original.push_str(line);
                body.push_str(line);
            }
            continue;
        }

        if let Some((delimiter, length)) = fence_at(content.as_bytes(), 0) {
            match fence {
                Some((open_delimiter, open_length))
                    if delimiter == open_delimiter && length >= open_length =>
                {
                    fence = None;
                }
                None => fence = Some((delimiter, length)),
                _ => {}
            }
            output.push_str(line);
            continue;
        }

        if fence.is_some() {
            output.push_str(line);
            continue;
        }

        if trimmed == "$$" {
            pending = Some(line.to_owned());
            continue;
        }

        if let Some(source) = trimmed
            .strip_prefix("$$")
            .and_then(|source| source.strip_suffix("$$"))
            .map(str::trim)
            .filter(|source| !source.is_empty())
        {
            output.push_str("```math\n");
            output.push_str(source);
            output.push_str("\n```");
            if line.ends_with('\n') {
                output.push('\n');
            }
            continue;
        }

        output.push_str(line);
    }

    if let Some(original) = pending {
        output.push_str(&original);
    }

    output
}

pub(super) struct Viewer;

impl<'a> markdown::Viewer<'a, Input> for Viewer {
    fn on_link_click(url: markdown::Uri) -> Input {
        Input::LinkClicked(url)
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        content: &markdown::Text,
    ) -> Element<'a, Input> {
        if let Some(source) = display_math(content, settings.style) {
            return math(&source, true, settings.text_size);
        }

        if has_inline_math(content, settings.style) {
            inline_content(content, settings, settings.text_size)
        } else {
            markdown::paragraph(settings, content, Self::on_link_click)
        }
    }

    fn heading(
        &self,
        settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        content: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Input> {
        let size = heading_size(settings, *level);

        if has_inline_math(content, settings.style) {
            container(inline_content(content, settings, size))
                .padding(iced::padding::top(if index > 0 {
                    settings.text_size / 2.0
                } else {
                    Pixels::ZERO
                }))
                .into()
        } else {
            markdown::heading(settings, level, content, index, Self::on_link_click)
        }
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        language: Option<&'a str>,
        code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, Input> {
        if language.is_some_and(is_math_language) {
            math(code.trim(), true, settings.text_size)
        } else {
            markdown::code_block(settings, lines, Self::on_link_click)
        }
    }
}

fn inline_content<'a>(
    content: &markdown::Text,
    settings: markdown::Settings,
    size: Pixels,
) -> Element<'a, Input> {
    let mut elements = Vec::new();

    for span in content.spans(settings.style).iter() {
        for fragment in inline_fragments(&span.text) {
            match fragment {
                InlineFragment::Text(source) => {
                    for text_fragment in split_text(source) {
                        let mut styled = span.clone();
                        styled.text = Cow::Owned(text_fragment.replace(ESCAPED_DOLLAR, "$"));

                        elements.push(
                            rich_text(vec![styled])
                                .on_link_click(Input::LinkClicked)
                                .size(size)
                                .into(),
                        );
                    }
                }
                InlineFragment::Math(source) => {
                    elements.push(math(source, false, size));
                }
            }
        }
    }

    row(elements)
        .width(Fill)
        .align_y(Center)
        .wrap()
        .vertical_spacing(0)
        .into()
}

fn math<'a>(source: &str, display: bool, size: Pixels) -> Element<'a, Input> {
    let renderer = iced_math::MathRenderer::new()
        .font_size(size.0)
        .display_style(display)
        .color(style::math_color());

    match renderer.to_svg(source) {
        Ok(bytes) => {
            let equation: Element<'a, Input> = svg::Svg::new(svg::Handle::from_memory(bytes))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into();

            if display {
                container(equation)
                    .center_x(Fill)
                    .padding(style::MATH_BLOCK_PADDING)
                    .into()
            } else {
                equation
            }
        }
        Err(_) => text(source.to_owned())
            .font(iced::Font::MONOSPACE)
            .size(size)
            .color(style::DANGER_COLOR)
            .into(),
    }
}

fn display_math(content: &markdown::Text, style: markdown::Style) -> Option<String> {
    let source = content
        .spans(style)
        .iter()
        .map(|span| span.text.as_ref())
        .collect::<String>();
    let source = source.trim();
    let inner = source.strip_prefix("$$")?.strip_suffix("$$")?.trim();

    (!inner.is_empty()).then(|| inner.to_owned())
}

fn has_inline_math(content: &markdown::Text, style: markdown::Style) -> bool {
    content.spans(style).iter().any(|span| {
        span.text.contains(ESCAPED_DOLLAR)
            || inline_fragments(&span.text)
                .any(|fragment| matches!(fragment, InlineFragment::Math(_)))
    })
}

fn inline_fragments(source: &str) -> impl Iterator<Item = InlineFragment<'_>> {
    let mut fragments = Vec::new();
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut search = 0;

    while search < bytes.len() {
        if bytes[search] != b'$'
            || search > 0 && bytes[search - 1] == b'$'
            || bytes.get(search + 1) == Some(&b'$')
            || bytes.get(search + 1).is_none_or(u8::is_ascii_whitespace)
        {
            search += 1;
            continue;
        }

        let mut close = search + 1;
        let mut closing = None;

        while close < bytes.len() {
            if bytes[close] == b'$'
                && bytes[close - 1] != b'$'
                && bytes.get(close + 1) != Some(&b'$')
                && !bytes[close - 1].is_ascii_whitespace()
                && bytes
                    .get(close + 1)
                    .is_none_or(|next| !next.is_ascii_digit())
            {
                closing = Some(close);
                break;
            }

            close += 1;
        }

        let Some(close) = closing else {
            search += 1;
            continue;
        };

        if cursor < search {
            fragments.push(InlineFragment::Text(&source[cursor..search]));
        }

        fragments.push(InlineFragment::Math(&source[search + 1..close]));
        cursor = close + 1;
        search = cursor;
    }

    if cursor < source.len() {
        fragments.push(InlineFragment::Text(&source[cursor..]));
    }

    fragments.into_iter()
}

fn split_text(source: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let mut fragments = Vec::new();

    for (index, character) in source.char_indices() {
        if character.is_whitespace() {
            let end = index + character.len_utf8();
            fragments.push(&source[start..end]);
            start = end;
        }
    }

    if start < source.len() {
        fragments.push(&source[start..]);
    }

    fragments
        .into_iter()
        .filter(|fragment| !fragment.is_empty())
}

fn heading_size(settings: markdown::Settings, level: markdown::HeadingLevel) -> Pixels {
    match level {
        markdown::HeadingLevel::H1 => settings.h1_size,
        markdown::HeadingLevel::H2 => settings.h2_size,
        markdown::HeadingLevel::H3 => settings.h3_size,
        markdown::HeadingLevel::H4 => settings.h4_size,
        markdown::HeadingLevel::H5 => settings.h5_size,
        markdown::HeadingLevel::H6 => settings.h6_size,
    }
}

fn is_math_language(language: &str) -> bool {
    matches!(language.trim(), "math" | "latex" | "tex")
}

fn fence_at(bytes: &[u8], line_start: usize) -> Option<(u8, usize)> {
    let mut index = line_start;
    let mut spaces = 0;

    while bytes.get(index) == Some(&b' ') && spaces < 3 {
        spaces += 1;
        index += 1;
    }

    let delimiter = *bytes.get(index)?;
    if !matches!(delimiter, b'`' | b'~') {
        return None;
    }

    let length = delimiter_run(bytes, index, delimiter);
    (length >= 3).then_some((delimiter, length))
}

fn delimiter_run(bytes: &[u8], start: usize, delimiter: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|byte| **byte == delimiter)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_llm_math_delimiters() {
        assert_eq!(
            normalize_delimiters(r"Inline \(x^2\) and display \[\frac{1}{2}\]"),
            "Inline $x^2$ and display $$\\frac{1}{2}$$"
        );
    }

    #[test]
    fn complete_display_math_blocks_become_math_code_blocks() {
        assert_eq!(
            normalize_delimiters("Before\n\n$$\nx_1 + x_2\n$$\n\nAfter"),
            "Before\n\n```math\nx_1 + x_2\n```\n\nAfter"
        );
        assert_eq!(
            normalize_delimiters("$$x_1 + x_2$$\n"),
            "```math\nx_1 + x_2\n```\n"
        );
    }

    #[test]
    fn incomplete_display_math_blocks_remain_plain_text() {
        assert_eq!(normalize_delimiters("$$\nx_1 + x_2"), "$$\nx_1 + x_2");
    }

    #[test]
    fn leaves_math_delimiters_inside_code_untouched() {
        let code = "`\\(inline code\\)`\n\n```text\n\\[fenced code\\]\n```";
        assert_eq!(normalize_delimiters(code), code);
        assert_eq!(
            normalize_delimiters("```text\n\\(code\\)\n```\n\\(math\\)"),
            "```text\n\\(code\\)\n```\n$math$"
        );
    }

    #[test]
    fn escaped_delimiters_remain_literal() {
        assert_eq!(normalize_delimiters(r"\\(not math\\)"), r"\\(not math\\)");
    }

    #[test]
    fn finds_inline_math_without_treating_prices_as_math() {
        assert_eq!(
            inline_fragments("Euler: $e^{i\\pi} + 1 = 0$.").collect::<Vec<_>>(),
            vec![
                InlineFragment::Text("Euler: "),
                InlineFragment::Math("e^{i\\pi} + 1 = 0"),
                InlineFragment::Text("."),
            ]
        );
        assert_eq!(
            inline_fragments("It costs $5 or $10.").collect::<Vec<_>>(),
            vec![InlineFragment::Text("It costs $5 or $10.")]
        );
        assert_eq!(
            inline_fragments("Not a block: $$x^2$$").collect::<Vec<_>>(),
            vec![InlineFragment::Text("Not a block: $$x^2$$")]
        );
    }

    #[test]
    fn native_renderer_produces_svg() {
        let svg = iced_math::MathRenderer::new()
            .to_svg(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}")
            .expect("supported LaTeX");
        let text_svg = iced_math::MathRenderer::new()
            .to_svg(r"P(\text{heads}) = 0.5")
            .expect("text in a formula");

        assert!(svg.starts_with(b"<svg"));
        assert!(text_svg.starts_with(b"<svg"));
    }
}
