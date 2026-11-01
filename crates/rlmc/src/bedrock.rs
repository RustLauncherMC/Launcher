//! The Bedrock Edition page: Cinnabar's Play, Installations and Patch notes tabs.

use crate::launcher::{hero, Launcher, Tab};
use crate::theme;
use crate::widgets::{button, disabled, heading, label, progress_bar, release_notes, IntoDiv};
use gpui::{div, prelude::*, px, ClickEvent, Context, Div, FontWeight, SharedString};
use rlmc_core::cinnabar::{self, BuildInfo, Channel, Install};
use rlmc_core::download::Progress;
use rlmc_core::github::Release;
use std::sync::Arc;
use std::time::Duration;

/// What the Play button does when clicked.
type Action = fn(&mut Launcher, &mut Context<Launcher>);

#[derive(Default)]
pub struct Bedrock {
    /// Still looking for an installed copy.
    searching: bool,
    install: Option<Install>,
    /// What the installed game says its version is.
    build: Option<Result<BuildInfo, String>>,
    /// The newest release on the selected channel, once fetched.
    release: Option<Result<Release, String>>,
    /// An install in progress.
    job: Option<Arc<Progress>>,
    running: bool,
    /// The outcome of the last install or game session.
    message: Option<Result<String, String>>,
}

/// The outermost message, which is written for players; the full chain goes to the log.
fn err(e: anyhow::Error) -> String {
    crate::widgets::friendly(e)
}

impl Launcher {
    /// Looks for an installed Cinnabar and asks it for its version.
    pub fn find_bedrock(&mut self, cx: &mut Context<Self>) {
        self.bedrock.searching = true;
        let task = cx.background_executor().spawn(async move {
            let install = cinnabar::find();
            let build = install
                .as_ref()
                .map(|i| cinnabar::build_info(i).map_err(err));
            (install, build)
        });
        cx.spawn(async move |this, cx| {
            let (install, build) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.bedrock.searching = false;
                this.bedrock.install = install;
                this.bedrock.build = build;
                cx.notify();
            });
        })
        .detach();
    }

    /// Fetches the newest release on the selected channel, for Install and Patch notes.
    pub fn fetch_bedrock_release(&mut self, cx: &mut Context<Self>) {
        let channel = self.settings.cinnabar_channel;
        let agent = self.agent.clone();
        self.bedrock.release = None;
        let task = cx
            .background_executor()
            .spawn(async move { cinnabar::latest(&agent, channel).map_err(err) });
        cx.spawn(async move |this, cx| {
            let release = task.await;
            let _ = this.update(cx, |this, cx| {
                // The player may have switched channel while this was loading.
                if this.settings.cinnabar_channel == channel {
                    this.bedrock.release = Some(release);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn set_bedrock_channel(&mut self, channel: Channel, cx: &mut Context<Self>) {
        if self.settings.cinnabar_channel == channel {
            return;
        }
        self.settings.cinnabar_channel = channel;
        self.save_settings();
        self.fetch_bedrock_release(cx);
        cx.notify();
    }

    fn install_bedrock(&mut self, cx: &mut Context<Self>) {
        if self.bedrock.job.is_some() || self.bedrock.running {
            return;
        }
        let progress = Arc::new(Progress::default());
        progress.set_step("Finding the latest release");
        self.bedrock.job = Some(progress.clone());
        self.bedrock.message = None;
        let agent = self.agent.clone();
        let channel = self.settings.cinnabar_channel;
        let known = match &self.bedrock.release {
            Some(Ok(release)) => Some(release.clone()),
            _ => None,
        };
        let job = progress.clone();
        let task = cx.background_executor().spawn(async move {
            let release = match known {
                Some(release) => release,
                None => cinnabar::latest(&agent, channel)?,
            };
            let install = cinnabar::install(&agent, &release, &job)?;
            let build = cinnabar::build_info(&install).map_err(err);
            anyhow::Ok((release, install, build))
        });
        // Redraw while the job runs, so the progress bar moves.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let busy = this
                .update(cx, |this, cx| {
                    cx.notify();
                    this.bedrock.job.is_some()
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
                this.bedrock.job = None;
                match done {
                    Ok((release, install, build)) => {
                        this.bedrock.message =
                            Some(Ok(format!("Installed Cinnabar {}", release.title())));
                        this.bedrock.install = Some(install);
                        this.bedrock.build = Some(build);
                    }
                    Err(e) => this.bedrock.message = Some(Err(err(e))),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn play_bedrock(&mut self, cx: &mut Context<Self>) {
        let Some(install) = self.bedrock.install.clone() else {
            return;
        };
        if self.bedrock.running || self.bedrock.job.is_some() {
            return;
        }
        let mut child = match cinnabar::launch(&install) {
            Ok(child) => child,
            Err(e) => {
                self.bedrock.message = Some(Err(err(e)));
                cx.notify();
                return;
            }
        };
        self.bedrock.running = true;
        self.bedrock.message = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move { child.wait() });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            let _ = this.update(cx, |this, cx| {
                this.bedrock.running = false;
                this.bedrock.message = match status {
                    Ok(s) if s.success() => None,
                    Ok(s) => Some(Err(match s.code() {
                        Some(code) => format!(
                            "Cinnabar closed with error code {code}. Its logs are in the game \
                             data folder (Settings)."
                        ),
                        None => "Cinnabar was stopped.".to_string(),
                    })),
                    Err(e) => Some(Err(e.to_string())),
                };
                // The game may have updated itself while it ran.
                this.find_bedrock(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn bedrock_page(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = [
            (Tab::Play, "Play"),
            (Tab::Installations, "Installations"),
            (Tab::PatchNotes, "Patch notes"),
        ];
        let body = match self.tab {
            Tab::Play => div()
                .flex_1()
                .flex()
                .flex_col()
                .child(self.bedrock_hero())
                .child(self.bedrock_bar(cx)),
            Tab::Installations => self.bedrock_installations(cx),
            Tab::PatchNotes => self.bedrock_notes(cx),
        };
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(self.header("BEDROCK EDITION", "Cinnabar", &tabs, cx))
            .child(body)
    }

    fn bedrock_hero(&self) -> Div {
        hero(theme::bedrock(), self.logo.clone())
            .child(heading("Cinnabar", 30.))
            .child(label(
                "A Bedrock Edition client written from scratch in Rust",
                15.,
                theme::muted(),
            ))
            .child(
                label(
                    "Sign in, pick an account and join worlds, Realms and servers in the game's \
                     own menu. Cinnabar is still in development: some servers' anticheats may \
                     flag it, so play on servers at your own risk.",
                    13.,
                    theme::faint(),
                )
                .max_w(px(560.))
                .text_center(),
            )
    }

    /// The bottom bar: channel and installed version, Play, and status.
    fn bedrock_bar(&self, cx: &mut Context<Self>) -> Div {
        let b = &self.bedrock;
        let supported = cinnabar::asset_name().is_some();

        let installed = match (&b.install, &b.build) {
            (None, _) if b.searching => "Looking for Cinnabar…".to_string(),
            (None, _) => "Not installed".to_string(),
            (Some(_), Some(Ok(build))) => format!("Installed: {}", build.describe()),
            (Some(_), _) => "Installed".to_string(),
        };
        let mut channels = div().flex().gap(px(4.));
        for option in Channel::ALL {
            let selected = option == self.settings.cinnabar_channel;
            channels = channels.child(
                div()
                    .id(SharedString::from(format!(
                        "bar-channel-{}",
                        option.label()
                    )))
                    .px(px(10.))
                    .py(px(3.))
                    .rounded(px(3.))
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(selected, |d| {
                        d.bg(theme::panel_hover()).text_color(theme::text())
                    })
                    .when(!selected, |d| {
                        d.text_color(theme::muted()).hover(|s| s.bg(theme::panel()))
                    })
                    .child(option.label())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_bedrock_channel(option, cx)
                    })),
            );
        }
        let left = div()
            .w(px(260.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(channels)
            .child(label(installed, 13., theme::muted()));

        let (text, action): (&str, Option<Action>) = if !supported {
            ("NOT AVAILABLE", None)
        } else if b.job.is_some() {
            ("INSTALLING", None)
        } else if b.running {
            ("PLAYING", None)
        } else if b.install.is_some() {
            ("PLAY", Some(Launcher::play_bedrock))
        } else if b.searching {
            ("PLAY", None)
        } else {
            ("INSTALL", Some(Launcher::install_bedrock))
        };
        let play = div()
            .id("bedrock-play")
            .w(px(260.))
            .h(px(58.))
            .rounded(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(20.))
            .font_weight(FontWeight::BOLD)
            .child(text);
        let play = match action {
            Some(run) => play
                .bg(theme::play())
                .text_color(theme::text())
                .cursor_pointer()
                .hover(|s| s.bg(theme::play_hover()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| run(this, cx))),
            None => play.bg(theme::play_disabled()).text_color(theme::muted()),
        };

        let right = div().w(px(260.)).flex().flex_col().items_end().gap(px(6.));
        let right = if let Some(job) = &b.job {
            let step = match job.fraction() {
                Some(f) => format!("{} · {:.0}%", job.step(), f * 100.),
                None => job.step(),
            };
            right
                .child(label(step, 12., theme::muted()).text_right())
                .child(progress_bar(job.fraction(), 220.))
        } else if !supported {
            right.child(
                label(
                    "Cinnabar has no release for this computer.",
                    12.,
                    theme::muted(),
                )
                .text_right(),
            )
        } else if let Some(message) = &b.message {
            match message {
                Ok(text) => right.child(label(text.clone(), 12., theme::muted()).text_right()),
                Err(text) => right.child(label(text.clone(), 12., theme::error()).text_right()),
            }
        } else {
            right.child(self.release_line())
        };

        div()
            .h(px(96.))
            .px(px(32.))
            .flex()
            .items_center()
            .justify_between()
            .bg(theme::bar_bg())
            .border_t_1()
            .border_color(theme::border())
            .child(left)
            .child(play)
            .child(right)
    }

    fn release_line(&self) -> Div {
        let channel = self.settings.cinnabar_channel.label();
        let text = match &self.bedrock.release {
            None => format!("Checking for the latest {channel}…"),
            Some(Ok(release)) => match release.date() {
                Some(date) => format!("Latest {channel}: {} · {date}", release.title()),
                None => format!("Latest {channel}: {}", release.title()),
            },
            Some(Err(_)) => "Could not reach GitHub".to_string(),
        };
        label(text, 12., theme::muted()).text_right()
    }

    fn bedrock_installations(&self, cx: &mut Context<Self>) -> Div {
        let b = &self.bedrock;
        let card = div()
            .p(px(16.))
            .rounded(px(6.))
            .bg(theme::panel())
            .flex()
            .items_center()
            .gap(px(16.))
            .child(
                crate::widgets::edition_icon(theme::bedrock())
                    .size(px(44.))
                    .flex_none(),
            );
        let card = match &b.install {
            Some(install) => {
                let version = match &b.build {
                    Some(Ok(build)) => build.describe(),
                    Some(Err(e)) => format!("Version unknown ({e})"),
                    None => "Reading version…".to_string(),
                };
                let folder = install.folder.clone();
                card.child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(heading("Cinnabar", 16.))
                        .child(label(version, 13., theme::muted()))
                        .child(crate::widgets::path_label(
                            install.folder.display().to_string(),
                        )),
                )
                .child(
                    button("install-open", "Open folder")
                        .on_click(move |_, _, cx| cx.open_with_system(&folder)),
                )
                .child(self.reinstall_button("Reinstall latest", cx))
            }
            None => card
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(heading("Cinnabar", 16.))
                        .child(label(
                            if b.searching {
                                "Looking for an installed copy…"
                            } else {
                                "Not installed yet"
                            },
                            13.,
                            theme::muted(),
                        )),
                )
                .child(self.reinstall_button("Install", cx)),
        };
        div()
            .id("installations-scroll")
            .flex_1()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(card)
            .child(label(
                "Cinnabar updates itself when it starts, on the channel it was installed from. \
                 Reinstall to switch channel or repair an install. Your worlds, packs and \
                 accounts are kept.",
                13.,
                theme::muted(),
            ))
            .child(label(
                "Separate installations, each with its own worlds and settings, are planned.",
                13.,
                theme::faint(),
            ))
            .into_any_element_div()
    }

    fn reinstall_button(&self, text: &'static str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let b = &self.bedrock;
        if b.job.is_some() || b.running || cinnabar::asset_name().is_none() {
            return disabled(text).into_any_element();
        }
        button(SharedString::from(format!("install-{text}")), text)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.install_bedrock(cx)))
            .into_any_element()
    }

    fn bedrock_notes(&self, cx: &mut Context<Self>) -> Div {
        let retry = button("notes-retry", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.fetch_bedrock_release(cx)))
            .into_any_element();
        release_notes(
            self.bedrock.release.as_ref(),
            self.settings.cinnabar_channel.label(),
            cinnabar::RELEASES_URL,
            retry,
        )
    }
}
