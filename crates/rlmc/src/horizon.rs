//! Horizon on the Java Edition page: Play (account check, install or update, prepare, launch),
//! Installations and Patch notes. Signing in is in `java.rs`.

use crate::launcher::{Launcher, Tab};
use crate::theme;
use crate::widgets::{button, heading, label, path_label, release_notes, IntoDiv};
use gpui::{div, prelude::*, px, ClickEvent, Context, Div, SharedString, Window};
use rlmc_core::accounts::{self, Keyring, Launch};
use rlmc_core::download::Progress;
use rlmc_core::github::Release;
use rlmc_core::horizon::{self, launch, prepare, Build};
use rlmc_core::settings::Edition;
use std::sync::Arc;
use std::time::Duration;

pub struct Horizon {
    pub builds: Vec<Build>,
    /// The newest nightly, once fetched.
    pub release: Option<Result<Release, String>>,
    /// Getting ready to play: account check, install, preparation, start.
    pub job: Option<Arc<Progress>>,
    pub running: bool,
    /// The outcome of the last start or game session.
    pub message: Option<Result<String, String>>,
}

impl Default for Horizon {
    fn default() -> Self {
        Horizon {
            builds: horizon::installed(),
            release: None,
            job: None,
            running: false,
            message: None,
        }
    }
}

/// The build to play: the newest nightly, installed if needed. When the update fails, or GitHub
/// can't be reached, the installed build plays instead.
fn pick_build(
    agent: &rlmc_core::ureq::Agent,
    latest: Option<Release>,
    current: Option<Build>,
    progress: &Progress,
) -> anyhow::Result<Build> {
    use anyhow::Context as _;
    let Some(release) = latest else {
        return current.context("could not reach GitHub to download Horizon");
    };
    if let Some(current) = current
        .as_ref()
        .filter(|b| b.version == horizon::version_of(&release))
    {
        return Ok(current.clone());
    }
    match (horizon::install(agent, &release, progress), current) {
        (Ok(build), _) => Ok(build),
        (Err(e), Some(current)) => {
            eprintln!("RustLauncherMC: Horizon update failed, playing the installed build: {e:#}");
            Ok(current)
        }
        (Err(e), None) => Err(e),
    }
}

fn err(e: anyhow::Error) -> String {
    crate::widgets::friendly(e)
}

impl Launcher {
    pub fn fetch_horizon_release(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        self.horizon.release = None;
        let task = cx
            .background_executor()
            .spawn(async move { horizon::latest(&agent).map_err(err) });
        cx.spawn(async move |this, cx| {
            let release = task.await;
            let _ = this.update(cx, |this, cx| {
                this.horizon.release = Some(release);
                cx.notify();
            });
        })
        .detach();
    }

    /// Play: checks the account, installs the newest build if needed, prepares the game files,
    /// and starts the game.
    pub fn play_java(&mut self, cx: &mut Context<Self>) {
        self.play_java_with(None, cx)
    }

    /// Plays the selected installation, joining `server` (`host` or `host:port`) when given.
    pub fn play_java_with(&mut self, server: Option<String>, cx: &mut Context<Self>) {
        if self.horizon.job.is_some() || self.horizon.running {
            return;
        }
        let Some(account) = self.java.accounts.selected().cloned() else {
            self.start_sign_in(cx);
            return;
        };
        let progress = Arc::new(Progress::default());
        self.horizon.job = Some(progress.clone());
        self.horizon.message = None;
        cx.notify();

        let agent = self.agent.clone();
        let latest = match &self.horizon.release {
            Some(Ok(release)) => Some(release.clone()),
            _ => None,
        };
        let installed = self.horizon.builds.clone();
        let installation = self.selected_installation(Edition::Java);
        let home = installation.folder().unwrap_or_else(horizon::home_dir);
        let start = launch::StartOptions {
            fullscreen: installation.fullscreen,
            server,
        };
        let bridge = self.java.horizon_launcher.clone();
        let job = progress.clone();
        let task = cx.background_executor().spawn(async move {
            // The owner gate: no download from Mojang and no start without an account that
            // owns Java Edition.
            job.set_step("Checking your account");
            let launch_as = if account.via_horizon {
                accounts::launch_via_horizon(bridge.as_ref(), &account)?
            } else {
                accounts::launch(&agent, &Keyring, &account)?
            };

            let latest = match latest {
                Some(release) => Some(release),
                None => horizon::latest(&agent).ok(),
            };
            let build = pick_build(&agent, latest, installed.first().cloned(), &job)?;

            prepare::ensure(&agent, &build, &job)?;

            let game = match &launch_as {
                Launch::Online(session) => {
                    job.set_step("Starting Horizon");
                    launch::start(&build, launch::Auth::Session(session), &home, &start)?
                }
                Launch::Offline(_) => {
                    job.set_step("Starting Horizon");
                    launch::start(&build, launch::Auth::Offline, &home, &start)?
                }
                Launch::Horizon(account) => {
                    // Asked for only now: the game refuses a session file older than two
                    // minutes, and preparing can take longer.
                    job.set_step("Getting your session from Horizon's launcher");
                    let bridge = bridge.as_ref().expect("checked by launch_via_horizon");
                    let file = bridge.session_file(&account.uuid)?;
                    job.set_step("Starting Horizon");
                    launch::start(&build, launch::Auth::SessionFile(&file), &home, &start)?
                }
            };
            let offline = matches!(launch_as, Launch::Offline(_));
            anyhow::Ok((game, offline))
        });
        // Redraw while the job runs, so the progress bar moves.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let busy = this
                .update(cx, |this, cx| {
                    cx.notify();
                    this.horizon.job.is_some()
                })
                .unwrap_or(false);
            if !busy {
                break;
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let started = task.await;
            let Ok(Some(game)) = this.update(cx, |this, cx| {
                this.horizon.job = None;
                this.horizon.builds = horizon::installed();
                cx.notify();
                match started {
                    Ok((game, offline)) => {
                        this.horizon.running = true;
                        if offline {
                            this.horizon.message =
                                Some(Ok("Minecraft's services could not be reached, so Horizon \
                                 started offline"
                                    .into()));
                        }
                        Some(game)
                    }
                    Err(e) => {
                        this.horizon.message = Some(Err(err(e)));
                        None
                    }
                }
            }) else {
                return;
            };
            let wait = cx.background_executor().spawn(async move {
                let mut game = game;
                let status = game.child.wait();
                (status, game.crash_report())
            });
            let (status, crash) = wait.await;
            let _ = this.update(cx, |this, cx| {
                this.horizon.running = false;
                this.horizon.message = match status.map(|s| s.code()) {
                    Ok(Some(0)) => None,
                    Ok(Some(2)) => Some(Err(
                        "Horizon refused how it was started. Updating RustLauncherMC may help"
                            .into(),
                    )),
                    Ok(Some(_)) => Some(Err(match crash {
                        Some(path) => format!("Horizon crashed. Crash report: {path}"),
                        None => "Horizon crashed. Its log is in the game folder's logs".into(),
                    })),
                    Ok(None) => Some(Err("Horizon was stopped".into())),
                    Err(e) => Some(Err(e.to_string())),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn java_tabs() -> [(Tab, &'static str); 3] {
        [
            (Tab::Play, "Play"),
            (Tab::Installations, "Installations"),
            (Tab::PatchNotes, "Patch notes"),
        ]
    }

    pub fn horizon_installations(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(10.));
        for (i, build) in self.horizon.builds.iter().enumerate() {
            let folder = build.dir.clone();
            list = list.child(
                div()
                    .p(px(16.))
                    .rounded(px(6.))
                    .bg(theme::panel())
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        crate::widgets::edition_icon(theme::java())
                            .size(px(44.))
                            .flex_none(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(heading(build.version.clone(), 16.))
                            .child(label(
                                if i == 0 {
                                    "Plays now"
                                } else {
                                    "Previous build, kept as a fallback"
                                },
                                13.,
                                theme::muted(),
                            ))
                            .child(path_label(build.dir.display().to_string())),
                    )
                    .child(
                        button(
                            SharedString::from(format!("build-open-{}", build.version)),
                            "Open folder",
                        )
                        .on_click(move |_, _, cx| cx.open_with_system(&folder)),
                    ),
            );
        }
        if self.horizon.builds.is_empty() {
            list = list.child(label(
                "Horizon is not installed yet. Play installs the newest nightly build.",
                13.,
                theme::muted(),
            ));
        }
        div()
            .id("horizon-installations")
            .flex_1()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p(px(32.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.installations_list(Edition::Java, window, cx))
            .child(heading("Horizon builds", 18.).mt(px(12.)))
            .child(list)
            .child(label(
                "Every installation plays the newest build. Play updates to the newest nightly; \
                 the build before it is kept.",
                13.,
                theme::muted(),
            ))
            .into_any_element_div()
    }

    pub fn horizon_notes(&self, cx: &mut Context<Self>) -> Div {
        let retry = button("notes-retry", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.fetch_horizon_release(cx)))
            .into_any_element();
        release_notes(
            self.horizon.release.as_ref(),
            "Nightly",
            horizon::RELEASES_URL,
            retry,
        )
    }

    /// The line under Play's progress: the newest nightly.
    pub fn horizon_release_line(&self) -> String {
        match &self.horizon.release {
            None => "Checking for the latest nightly…".into(),
            Some(Ok(release)) => format!("Latest: {}", horizon::version_of(release)),
            Some(Err(_)) => "Could not reach GitHub".into(),
        }
    }
}
