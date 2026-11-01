//! Small building blocks shared by the pages.

use crate::theme;
use gpui::{
    div, prelude::*, px, AnyElement, Div, ElementId, FontWeight, Hsla, SharedString, Stateful,
};
use rlmc_core::github::Release;

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

/// The main action of a screen, in the Play button's green.
pub fn primary_button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .px(px(16.))
        .py(px(8.))
        .rounded(px(4.))
        .bg(theme::play())
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(theme::play_hover()))
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

/// An error's outermost message, for players, starting with a capital letter. The full chain
/// goes to the log.
pub fn friendly(e: anyhow::Error) -> String {
    eprintln!("RustLauncherMC: {e:#}");
    let text = e.to_string();
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// A one-line label that ends in "…" when it doesn't fit, for paths.
pub fn path_label(text: impl Into<SharedString>) -> Div {
    label(text, 12., theme::faint())
        .w_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
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

/// Scrolling containers need an id, which makes them `Stateful<Div>`; the pages are plain
/// `Div`s, so wrap them.
pub trait IntoDiv {
    fn into_any_element_div(self) -> Div;
}

impl IntoDiv for Stateful<Div> {
    fn into_any_element_div(self) -> Div {
        div().flex_1().flex().flex_col().min_h(px(0.)).child(self)
    }
}

/// A Patch notes tab: the release's title, channel and date, a link to GitHub, and its notes.
pub fn release_notes(
    release: Option<&Result<Release, String>>,
    channel: &str,
    releases_url: &str,
    retry: AnyElement,
) -> Div {
    let content = match release {
        None => label("Loading release notes…", 14., theme::muted()),
        Some(Err(e)) => div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(label("Could not load release notes.", 14., theme::text()))
            .child(label(e.clone(), 12., theme::faint()))
            .child(div().flex().child(retry)),
        Some(Ok(release)) => {
            let url = release
                .html_url
                .clone()
                .unwrap_or_else(|| releases_url.to_string());
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(heading(release.title().to_string(), 22.))
                                .child(label(
                                    format!("{channel} · {}", release.date().unwrap_or("")),
                                    13.,
                                    theme::muted(),
                                )),
                        )
                        .child(
                            button("notes-github", "View on GitHub")
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        ),
                )
                .child(div().h(px(1.)).bg(theme::border()))
                .child(
                    match release.body.as_deref().filter(|b| !b.trim().is_empty()) {
                        Some(body) => notes(body),
                        None => label("This release has no notes.", 14., theme::muted()),
                    },
                )
        }
    };
    div()
        .id("notes-scroll")
        .flex_1()
        .overflow_y_scroll()
        .overflow_x_hidden()
        .px(px(32.))
        .py(px(24.))
        .child(content)
        .into_any_element_div()
}

/// A one-line text field: typing, Backspace, paste, Enter and Escape. GPUI has no text input of
/// its own; this covers names and server addresses (no cursor movement or selection).
pub struct Field {
    pub text: String,
    pub focus: gpui::FocusHandle,
}

/// What a key press in a [`Field`] asks for.
pub enum FieldKey {
    Edited,
    Submit,
    Cancel,
    Ignored,
}

impl Field {
    pub fn new(text: impl Into<String>, cx: &mut gpui::App) -> Self {
        Field {
            text: text.into(),
            focus: cx.focus_handle(),
        }
    }

    pub fn key(&mut self, event: &gpui::KeyDownEvent, cx: &mut gpui::App) -> FieldKey {
        let keystroke = &event.keystroke;
        let command = keystroke.modifiers.control || keystroke.modifiers.platform;
        match keystroke.key.as_str() {
            "enter" => return FieldKey::Submit,
            "escape" => return FieldKey::Cancel,
            "backspace" => {
                self.text.pop();
                return FieldKey::Edited;
            }
            "v" if command => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.text.push_str(text.lines().next().unwrap_or("").trim());
                    self.text.truncate(200);
                }
                return FieldKey::Edited;
            }
            _ => {}
        }
        match &keystroke.key_char {
            Some(typed) if !command && self.text.len() < 200 => {
                self.text.extend(typed.chars().filter(|c| !c.is_control()));
                FieldKey::Edited
            }
            _ => FieldKey::Ignored,
        }
    }

    /// The field, with a caret while it has focus. The caller adds `on_key_down`.
    pub fn view(
        &self,
        id: impl Into<ElementId>,
        placeholder: &str,
        width: f32,
        window: &gpui::Window,
    ) -> Stateful<Div> {
        let focused = self.focus.is_focused(window);
        let shown = if self.text.is_empty() && !focused {
            label(placeholder.to_string(), 13., theme::faint())
        } else {
            label(
                format!("{}{}", self.text, if focused { "▏" } else { "" }),
                13.,
                theme::text(),
            )
        };
        div()
            .id(id.into())
            .track_focus(&self.focus)
            .w(px(width))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(4.))
            .bg(theme::sidebar_bg())
            .border_1()
            .border_color(if focused {
                theme::play_hover()
            } else {
                theme::border()
            })
            .overflow_hidden()
            .whitespace_nowrap()
            .cursor_text()
            .child(shown)
    }
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
