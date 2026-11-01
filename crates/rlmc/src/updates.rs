//! The launcher updating itself: a check at start, a banner in the sidebar when an update is
//! available, and Settings → Updates.

use crate::launcher::Launcher;
use crate::theme;
use crate::widgets::{button, disabled, label, progress_bar};
use gpui::{div, prelude::*, px, ClickEvent, Context, Div, FontWeight};
use rlmc_core::download::Progress;
use rlmc_core::github::Release;
use rlmc_core::update::{self, Target};
use std::sync::Arc;
use std::time::Duration;

pub enum Updates {
    Checking,
    UpToDate,
    Available(Release),
    Installing(Arc<Progress>),
    /// Installed; the new version runs after a restart.
    Ready(String),
    /// This copy can't update itself, and why.
    NotPossible(String),
    Failed(String),
}

impl Launcher {
    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(self.updates, Updates::Installing(_) | Updates::Ready(_)) {
            return;
        }
        if let Err(reason) = update::target() {
            self.updates = Updates::NotPossible(reason);
            cx.notify();
            return;
        }
        self.updates = Updates::Checking;
        cx.notify();
        let agent = self.agent.clone();
        let task = cx
            .background_executor()
            .spawn(async move { update::check(&agent) });
        cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates = match found {
                    Ok(Some(release)) => Updates::Available(release),
                    Ok(None) => Updates::UpToDate,
                    Err(e) => Updates::Failed(crate::widgets::friendly(e)),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let Updates::Available(release) = &self.updates else {
            return;
        };
        let target = match update::target() {
            Ok(target) => target,
            Err(reason) => {
                self.updates = Updates::NotPossible(reason);
                cx.notify();
                return;
            }
        };
        let release = release.clone();
        let progress = Arc::new(Progress::default());
        self.updates = Updates::Installing(progress.clone());
        cx.notify();
        let agent = self.agent.clone();
        let job = progress.clone();
        let tag = release.tag_name.clone();
        let task = cx
            .background_executor()
            .spawn(async move { update::install(&agent, &release, &target, &job) });
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let busy = this
                .update(cx, |this, cx| {
                    cx.notify();
                    matches!(this.updates, Updates::Installing(_))
                })
                .unwrap_or(false);
            if !busy {
                break;
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updates = match done {
                    Ok(()) => Updates::Ready(tag),
                    Err(e) => Updates::Failed(crate::widgets::friendly(e)),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        let restarted = update::target()
            .map_err(anyhow::Error::msg)
            .and_then(|target: Target| update::restart(&target));
        match restarted {
            Ok(()) => cx.quit(),
            Err(e) => {
                self.updates = Updates::Failed(crate::widgets::friendly(e));
                cx.notify();
            }
        }
    }

    /// The sidebar banner, shown only when there is something to do.
    pub fn update_banner(&self, cx: &mut Context<Self>) -> Option<Div> {
        let (text, action): (String, fn(&mut Launcher, &mut Context<Launcher>)) =
            match &self.updates {
                Updates::Available(release) => (
                    format!("Update to {}", release.tag_name),
                    Launcher::install_update,
                ),
                Updates::Ready(tag) => (
                    format!("Restart to finish updating to {tag}"),
                    Launcher::restart_to_update,
                ),
                Updates::Installing(progress) => {
                    return Some(
                        banner_box()
                            .child(label("Updating RustLauncherMC…", 12., theme::text()))
                            .child(progress_bar(progress.fraction(), 200.)),
                    );
                }
                _ => return None,
            };
        Some(
            banner_box().child(
                div()
                    .id("update-banner")
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .hover(|s| s.underline())
                    .child(text)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| action(this, cx))),
            ),
        )
    }

    /// Settings → Updates.
    pub fn updates_settings(&self, cx: &mut Context<Self>) -> Div {
        let status = match &self.updates {
            Updates::Checking => "Checking for updates…".to_string(),
            Updates::UpToDate => "RustLauncherMC is up to date.".to_string(),
            Updates::Available(r) => format!("{} is available.", r.tag_name),
            Updates::Installing(p) => p.step(),
            Updates::Ready(tag) => format!("{tag} is installed. Restart to use it."),
            Updates::NotPossible(reason) | Updates::Failed(reason) => reason.clone(),
        };
        let color = match &self.updates {
            Updates::Failed(_) => theme::error(),
            _ => theme::muted(),
        };
        let action = match &self.updates {
            Updates::Available(_) => button("update-install", "Update now")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.install_update(cx)))
                .into_any_element(),
            Updates::Ready(_) => button("update-restart", "Restart now")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.restart_to_update(cx)))
                .into_any_element(),
            Updates::Checking | Updates::Installing(_) | Updates::NotPossible(_) => {
                disabled("Check now").into_any_element()
            }
            Updates::UpToDate | Updates::Failed(_) => button("update-check", "Check now")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.check_for_updates(cx)))
                .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(label(
                format!("This is RustLauncherMC {}.", update::VERSION),
                13.,
                theme::text(),
            ))
            .child(label(status, 13., color))
            .child(
                div().flex().gap(px(8.)).child(action).child(
                    button("update-releases", "All releases")
                        .on_click(|_, _, cx| cx.open_url(update::RELEASES_URL)),
                ),
            )
    }
}

fn banner_box() -> Div {
    div()
        .mx(px(8.))
        .mb(px(8.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(4.))
        .bg(theme::play())
        .flex()
        .flex_col()
        .gap(px(6.))
}
