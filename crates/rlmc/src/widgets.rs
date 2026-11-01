//! Small building blocks shared by the pages.

use crate::theme;
use gpui::{div, prelude::*, px, Div, ElementId, FontWeight, Hsla, SharedString, Stateful};

/// A text block with the launcher's font size and colour.
pub fn label(text: impl Into<SharedString>, size: f32, color: impl Into<Hsla>) -> Div {
    div()
        .text_size(px(size))
        .text_color(color.into())
        .child(text.into())
}

pub fn heading(text: impl Into<SharedString>, size: f32) -> Div {
    label(text, size, theme::text()).font_weight(FontWeight::BOLD)
}

/// A secondary button: grey, with a lighter hover.
pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .px(px(14.))
        .py(px(7.))
        .rounded(px(4.))
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .text_size(px(13.))
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(theme::panel_hover()))
        .child(text.into())
}

/// A button that does nothing, shown in place of one that cannot be used right now.
pub fn disabled(text: impl Into<SharedString>) -> Div {
    div()
        .px(px(14.))
        .py(px(7.))
        .rounded(px(4.))
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .text_size(px(13.))
        .text_color(theme::faint())
        .child(text.into())
}

/// A thin progress bar; `fraction` is `None` while the size is unknown.
pub fn progress_bar(fraction: Option<f32>, width: f32) -> Div {
    let fill = fraction.unwrap_or(0.0).clamp(0.0, 1.0);
    div()
        .w(px(width))
        .h(px(4.))
        .rounded(px(2.))
        .bg(theme::panel())
        .child(
            div()
                .h_full()
                .w(px(width * fill))
                .rounded(px(2.))
                .bg(theme::play_hover()),
        )
}

/// A small isometric-looking edition icon: a square in the edition's colour with a grass top.
pub fn edition_icon(color: impl Into<Hsla>) -> Div {
    div()
        .size(px(28.))
        .rounded(px(4.))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().h(px(9.)).bg(theme::grass()))
        .child(div().flex_1().bg(color.into()))
}

/// Release notes are Markdown. This shows them readably without a full Markdown renderer:
/// headings in bold, list items with a bullet, and links and emphasis markers removed.
pub fn notes(markdown: &str) -> Div {
    let mut column = div().flex().flex_col().gap(px(6.));
    for raw in markdown.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            column = column.child(div().h(px(4.)));
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level = 1 + rest.chars().take_while(|c| *c == '#').count();
            let text = plain(rest.trim_start_matches('#').trim());
            let size = match level {
                1 => 20.,
                2 => 17.,
                _ => 15.,
            };
            column = column.child(heading(text, size).mt(px(6.)));
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            let indent = (line.len() - trimmed.len()) as f32 * 6.;
            column = column.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .pl(px(indent))
                    .child(label("•", 14., theme::muted()))
                    .child(label(plain(item), 14., theme::muted()).flex_1()),
            );
        } else {
            column = column.child(label(plain(trimmed), 14., theme::muted()));
        }
    }
    column
}

/// `[text](url)` becomes `text`, and `**`, `__` and backticks are dropped.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| {
            after[close + 2..]
                .find(')')
                .map(|end| (close, close + 2 + end))
        }) {
            Some((close, end)) => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace("__", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn strips_links_and_emphasis() {
        assert_eq!(plain("See [the docs](https://x/y) now"), "See the docs now");
        assert_eq!(plain("**Bold** and `code`"), "Bold and code");
        assert_eq!(plain("a [b] c"), "a [b] c");
        assert_eq!(plain("[a](u) [b](v)"), "a b");
    }
}
