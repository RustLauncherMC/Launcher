//! The Logs tab of both editions: the selected installation's logs and crash reports, and a
//! viewer with Copy, Open folder and Refresh. After a crash, the bottom bar links here.

use crate::launcher::{Launcher, Page, Tab};
use crate::theme;
use crate::widgets::{button, heading, label, path_label, IntoDiv};
use gpui::{
    div, prelude::*, px, ClickEvent, ClipboardItem, Context, Div, FontWeight, ScrollHandle,
    SharedString,
};
use rlmc_core::logs::{self, Kind, Level, LogFile};
use rlmc_core::settings::Edition;
use std::path::PathBuf;

/// Lines shown at most; the file's end is read, and its last lines shown.
const MAX_LINES: usize = 1500;

/// A file's text, or why it couldn't be read; `true` when only its end was read.
type Loaded = Result<(String, bool), String>;

#[derive(Default)]
pub struct LogsUi {
    /// The file being viewed.
    selected: Option<PathBuf>,
    /// Its text, read when it was selected or refreshed.
    loaded: Option<(PathBuf, Loaded)>,
    scroll: ScrollHandle,
    /// A crash to point at from the bottom bar: the edition, and its report when known.
    pub crash: Option<(Edition, Option<PathBuf>)>,
}

impl Launcher {
    /// Shows the Logs tab of `edition`, with `path` selected, or the newest crash report, or the
    /// first log.
    pub fn open_logs(&mut self, edition: Edition, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.page = Page::Edition(edition);
        self.tab = Tab::Logs;
        let files = logs::files(&self.selected_installation(edition));
        let pick = path
            .filter(|p| p.is_file())
            .or_else(|| {
                files
                    .iter()
                    .find(|f| f.kind == Kind::Crash)
                    .map(|f| f.path.clone())
            })
            .or_else(|| files.first().map(|f| f.path.clone()));
        self.select_log(pick, cx);
    }

    fn select_log(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.logs.selected = path.clone();
        self.logs.loaded = path.map(|p| {
            let text = logs::read_tail(&p).map_err(|e| e.to_string());
            (p, text)
        });
        // Logs grow at the end: show the newest lines first.
        self.logs.scroll.scroll_to_bottom();
        cx.notify();
    }

    /// The bottom bar's link to a crash of `edition`, if there was one.
    pub fn crash_link(&self, edition: Edition, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (crashed, report) = self.logs.crash.clone()?;
        if crashed != edition {
            return None;
        }
        let text = if report.is_some() {
            "View crash report"
        } else {
            "View logs"
        };
        Some(
            div()
                .id("crash-link")
                .text_size(px(12.))
                .text_color(theme::text())
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(text)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.open_logs(edition, report.clone(), cx)
                }))
                .into_any_element(),
        )
    }

    /// The Logs tab.
    pub fn logs_tab(&mut self, edition: Edition, cx: &mut Context<Self>) -> Div {
        let installation = self.selected_installation(edition);
        let files = logs::files(&installation);
        // A selection from another installation or edition doesn't apply here.
        if self
            .logs
            .selected
            .as_ref()
            .is_some_and(|s| !files.iter().any(|f| &f.path == s))
        {
            self.logs.selected = None;
            self.logs.loaded = None;
        }
        if self.logs.selected.is_none() {
            if let Some(first) = files.first() {
                self.select_log(Some(first.path.clone()), cx);
            }
        }

        let mut list = div().flex().flex_col().gap(px(2.));
        for file in &files {
            list = list.child(self.file_row(file, cx));
        }
        if files.is_empty() {
            let where_text = logs::root(&installation)
                .map(|r| r.display().to_string())
                .unwrap_or_default();
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .p(px(32.))
                .child(heading("No logs yet", 18.))
                .child(label(
                    format!(
                        "{}'s logs and crash reports appear here after you play it.",
                        installation.name
                    ),
                    13.,
                    theme::muted(),
                ))
                .child(path_label(where_text).max_w(px(560.)).text_center());
        }

        let sidebar = div()
            .id("logs-files")
            .w(px(250.))
            .flex_none()
            .overflow_y_scroll()
            .p(px(12.))
            .bg(theme::sidebar_bg())
            .border_r_1()
            .border_color(theme::border())
            .child(label(installation.name.clone(), 12., theme::faint()).mb(px(6.)))
            .child(list);

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .child(sidebar)
            .child(self.viewer(cx))
    }

    fn file_row(&self, file: &LogFile, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.logs.selected.as_ref() == Some(&file.path);
        let path = file.path.clone();
        // A crash's name is its time already.
        let when = match file.kind {
            Kind::Log => file
                .modified
                .and_then(logs::format_time)
                .unwrap_or_default(),
            Kind::Crash => "Crash report".to_string(),
        };
        let (icon, color) = match file.kind {
            Kind::Log => ("▤", theme::muted()),
            Kind::Crash => ("⚠", theme::error()),
        };
        div()
            .id(SharedString::from(format!("log-{}", file.path.display())))
            .flex()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .rounded(px(4.))
            .cursor_pointer()
            .when(selected, |d| d.bg(theme::sidebar_selected()))
            .when(!selected, |d| d.hover(|s| s.bg(theme::sidebar_hover())))
            .child(label(icon, 13., color))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        label(file.name.clone(), 13., theme::text())
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(label(when, 11., theme::faint())),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select_log(Some(path.clone()), cx)
            }))
    }

    fn viewer(&self, cx: &mut Context<Self>) -> Div {
        let Some((path, text)) = &self.logs.loaded else {
            return div().flex_1();
        };
        let folder = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let refresh = path.clone();
        let copied = match text {
            Ok((t, _)) => t.clone(),
            Err(_) => String::new(),
        };
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(path_label(path.display().to_string())),
            )
            .child(button("log-copy", "Copy").on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
            }))
            .child(
                button("log-folder", "Open folder")
                    .on_click(move |_, _, cx| cx.open_with_system(&folder)),
            )
            .child(button("log-refresh", "Refresh").on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| this.select_log(Some(refresh.clone()), cx),
            )));
        let body = match text {
            Err(e) => div().p(px(16.)).child(label(
                format!("Couldn't read this file: {e}"),
                13.,
                theme::error(),
            )),
            Ok((text, cut)) => {
                let lines: Vec<&str> = text.lines().collect();
                let skipped = lines.len().saturating_sub(MAX_LINES);
                let mut column = div().flex().flex_col().px(px(16.)).py(px(10.));
                if *cut || skipped > 0 {
                    column = column.child(
                        label(
                            "Showing the end of this file. Open its folder for all of it.",
                            12.,
                            theme::faint(),
                        )
                        .mb(px(6.)),
                    );
                }
                for line in &lines[skipped..] {
                    let color = match logs::level(line) {
                        Level::Error => theme::error(),
                        Level::Warn => gpui::rgb(0xe3b341),
                        Level::Plain => gpui::rgb(0xd6d0c8),
                    };
                    column = column.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(color)
                            .child(if line.is_empty() {
                                " ".to_string()
                            } else {
                                line.to_string()
                            }),
                    );
                }
                column
            }
        };
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .child(toolbar)
            .child(
                div()
                    .id("log-text")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.logs.scroll)
                    .child(body)
                    .into_any_element_div(),
            )
    }
}
