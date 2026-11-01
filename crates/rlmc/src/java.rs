//! The Java Edition page and Microsoft sign-in. Installing and starting Horizon is in
//! `horizon.rs`.

use crate::launcher::{hero, Launcher, Page, Tab};
use crate::theme;
use crate::widgets::{button, disabled, heading, label, primary_button, progress_bar};
use gpui::{
    div, prelude::*, px, ClickEvent, ClipboardItem, Context, Div, FontWeight, SharedString,
};
use rlmc_core::accounts::{self, Accounts, Keyring};
use rlmc_core::horizon;
use rlmc_core::microsoft::{self, DeviceCode};
use rlmc_core::settings::Edition;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What the Play button does when clicked.
type Action = fn(&mut Launcher, &mut Context<Launcher>);

const HORIZON_URL: &str = "https://github.com/horizon-rust/horizon";

pub struct Java {
    pub accounts: Accounts,
    sign_in: Option<SignIn>,
    /// The outcome of the last sign-in or sign-out.
    message: Option<Result<String, String>>,
    /// Player heads by UUID, once loaded; `None` while loading or when there is none.
    heads: std::collections::HashMap<String, Option<Arc<gpui::Image>>>,
}

enum SignIn {
    /// Asking Microsoft for a code.
    Starting,
    /// Waiting for the player to enter the code.
    Code {
        code: DeviceCode,
        cancel: Arc<AtomicBool>,
    },
    /// Checking the account with Xbox Live and Minecraft.
    Checking,
}

impl Default for Java {
    fn default() -> Self {
        Java {
            accounts: Accounts::load(),
            sign_in: None,
            message: None,
            heads: Default::default(),
        }
    }
}

fn err(e: anyhow::Error) -> String {
    crate::widgets::friendly(e)
}

impl Launcher {
    pub fn start_sign_in(&mut self, cx: &mut Context<Self>) {
        if self.java.sign_in.is_some() {
            return;
        }
        self.java.sign_in = Some(SignIn::Starting);
        self.java.message = None;
        self.page = Page::Edition(Edition::Java);
        self.tab = Tab::Play;
        cx.notify();

        let agent = self.agent.clone();
        let task = cx
            .background_executor()
            .spawn(async move { microsoft::device_code(&agent) });
        cx.spawn(async move |this, cx| {
            let code = task.await;
            let _ = this.update(cx, |this, cx| match code {
                Ok(code) => this.wait_for_code(code, cx),
                Err(e) => this.sign_in_failed(err(e), cx),
            });
        })
        .detach();
    }

    fn wait_for_code(&mut self, code: DeviceCode, cx: &mut Context<Self>) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.java.sign_in = Some(SignIn::Code {
            code: code.clone(),
            cancel: cancel.clone(),
        });
        cx.notify();

        let agent = self.agent.clone();
        let cancelled = cancel.clone();
        let wait = cx.background_executor().spawn({
            let agent = agent.clone();
            async move { microsoft::wait_for_sign_in(&agent, &code, &cancel) }
        });
        cx.spawn(async move |this, cx| {
            let tokens = match wait.await {
                Ok(tokens) => tokens,
                // Cancel already reset the page; a cancelled sign-in is not an error.
                Err(_) if cancelled.load(Ordering::Relaxed) => return,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| this.sign_in_failed(err(e), cx));
                    return;
                }
            };
            let Ok(mut accounts) = this.update(cx, |this, cx| {
                this.java.sign_in = Some(SignIn::Checking);
                cx.notify();
                this.java.accounts.clone()
            }) else {
                return;
            };
            let check = cx.background_executor().spawn(async move {
                let account = accounts::add(&agent, &Keyring, &mut accounts, &tokens)?;
                accounts.save()?;
                anyhow::Ok((account, accounts))
            });
            let done = check.await;
            let _ = this.update(cx, |this, cx| match done {
                Ok((account, accounts)) => {
                    this.java.accounts = accounts;
                    this.load_heads(cx);
                    this.java.sign_in = None;
                    this.java.message = Some(Ok(format!("Signed in as {}", account.name)));
                    cx.notify();
                }
                Err(e) => this.sign_in_failed(err(e), cx),
            });
        })
        .detach();
    }

    /// Loads the heads of signed-in players that aren't loaded yet.
    pub fn load_heads(&mut self, cx: &mut Context<Self>) {
        for account in self.java.accounts.accounts.clone() {
            if self.java.heads.contains_key(&account.uuid) {
                continue;
            }
            self.java.heads.insert(account.uuid.clone(), None);
            let agent = self.agent.clone();
            let uuid = account.uuid.clone();
            let task = cx
                .background_executor()
                .spawn(async move { rlmc_core::skins::head_png(&agent, &uuid) });
            let uuid = account.uuid;
            cx.spawn(async move |this, cx| {
                let png = task.await;
                let _ = this.update(cx, |this, cx| {
                    match png {
                        Ok(png) => {
                            let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, png);
                            this.java.heads.insert(uuid, Some(Arc::new(image)));
                        }
                        Err(e) => eprintln!("RustLauncherMC: no head for {uuid}: {e:#}"),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// A player's head, or a plain square until it has loaded.
    fn head(&self, uuid: &str, size: f32) -> gpui::AnyElement {
        match self.java.heads.get(uuid).cloned().flatten() {
            Some(image) => gpui::img(image)
                .size(px(size))
                .rounded(px(3.))
                .into_any_element(),
            None => div()
                .size(px(size))
                .rounded(px(3.))
                .bg(theme::panel_hover())
                .into_any_element(),
        }
    }

    fn sign_in_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.java.sign_in = None;
        self.java.message = Some(Err(message));
        cx.notify();
    }

    fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if let Some(SignIn::Code { cancel, .. }) = &self.java.sign_in {
            cancel.store(true, Ordering::Relaxed);
        }
        self.java.sign_in = None;
        self.java.message = None;
        cx.notify();
    }

    pub fn select_account(&mut self, uuid: String, cx: &mut Context<Self>) {
        self.java.accounts.selected = Some(uuid);
        if let Err(e) = self.java.accounts.save() {
            self.java.message = Some(Err(err(e)));
        }
        cx.notify();
    }

    pub fn sign_out(&mut self, uuid: String, cx: &mut Context<Self>) {
        let name = self
            .java
            .accounts
            .accounts
            .iter()
            .find(|a| a.uuid == uuid)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        let result = accounts::sign_out(&Keyring, &mut self.java.accounts, &uuid)
            .and_then(|()| self.java.accounts.save());
        self.java.message = Some(match result {
            Ok(()) => Ok(format!("Signed out {name}")),
            Err(e) => Err(err(e)),
        });
        cx.notify();
    }

    pub fn java_page(&mut self, window: &gpui::Window, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(self.header("JAVA EDITION", "Horizon", &Self::java_tabs(), cx))
            .child(match self.tab {
                Tab::Play => div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(self.java_hero(window, cx))
                    .child(self.java_bar(cx)),
                Tab::Installations => self.horizon_installations(window, cx),
                Tab::Logs => self.logs_tab(Edition::Java, cx),
                Tab::PatchNotes => self.horizon_notes(cx),
            })
    }

    fn java_hero(&self, window: &gpui::Window, cx: &mut Context<Self>) -> Div {
        let hero = hero(theme::java(), self.logo.clone());
        match &self.java.sign_in {
            Some(SignIn::Starting) => {
                hero.child(label("Contacting Microsoft…", 15., theme::muted()))
            }
            Some(SignIn::Checking) => hero.child(label(
                "Checking your Minecraft account…",
                15.,
                theme::muted(),
            )),
            Some(SignIn::Code { code, .. }) => {
                let user_code = code.user_code.clone();
                let url = code.verification_uri.clone();
                hero.child(heading("Sign in with Microsoft", 24.))
                    .child(label(
                        format!("Go to {} and enter this code:", display_url(&url)),
                        15.,
                        theme::muted(),
                    ))
                    .child(
                        div()
                            .px(px(22.))
                            .py(px(10.))
                            .rounded(px(6.))
                            .bg(theme::panel())
                            .border_1()
                            .border_color(theme::border())
                            .text_size(px(32.))
                            .font_weight(FontWeight::BOLD)
                            .child(code.user_code.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                primary_button("signin-open", "Open sign-in page")
                                    .on_click(move |_, _, cx| cx.open_url(&url)),
                            )
                            .child(
                                button("signin-copy", "Copy code").on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        user_code.clone(),
                                    ))
                                }),
                            )
                            .child(button("signin-cancel", "Cancel").on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.cancel_sign_in(cx)),
                            )),
                    )
                    .child(label("Waiting for you to sign in…", 13., theme::faint()))
            }
            None => match self.java.accounts.selected() {
                Some(account) => hero
                    .child(heading("Horizon", 30.))
                    .child(label(
                        "Minecraft: Java Edition rewritten in Rust",
                        15.,
                        theme::muted(),
                    ))
                    .child(
                        label(
                            format!(
                                "Playing as {}. The first start downloads Horizon and prepares \
                                 its game files from your copy of Minecraft, which takes a few \
                                 minutes.",
                                account.name
                            ),
                            13.,
                            theme::faint(),
                        )
                        .max_w(px(560.))
                        .text_center(),
                    )
                    .when(
                        self.horizon.job.is_none() && !self.horizon.running,
                        |hero| hero.child(self.join_server(Edition::Java, window, cx)),
                    )
                    .child(
                        button("java-horizon", "About Horizon")
                            .on_click(|_, _, cx| cx.open_url(HORIZON_URL)),
                    ),
                None => hero
                    .child(heading("Sign in to play Java Edition", 24.))
                    .child(
                        label(
                            "Horizon is Minecraft: Java Edition rewritten in Rust. It needs a \
                             Microsoft account that owns Minecraft: Java Edition.",
                            15.,
                            theme::muted(),
                        )
                        .max_w(px(560.))
                        .text_center(),
                    )
                    .child(
                        primary_button("signin-start", "Sign in with Microsoft").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.start_sign_in(cx)),
                        ),
                    ),
            },
        }
    }

    fn java_bar(&self, cx: &mut Context<Self>) -> Div {
        let left = div()
            .w(px(260.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .child(self.installation_picker(Edition::Java, cx)),
            )
            .child(match self.java.accounts.selected() {
                Some(account) => div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(self.head(&account.uuid, 20.))
                    .child(
                        label(account.name.clone(), 14., theme::text())
                            .font_weight(FontWeight::SEMIBOLD),
                    ),
                None => label("Not signed in", 14., theme::muted()),
            });
        let left = match self.java.accounts.selected() {
            Some(_) => left,
            None if self.java.sign_in.is_none() => left.child(
                div().flex().child(
                    div()
                        .id("bar-signin")
                        .text_size(px(12.))
                        .text_color(theme::play_hover())
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child("Sign in")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.start_sign_in(cx)),
                        ),
                ),
            ),
            None => left,
        };
        let h = &self.horizon;
        let installed = h.builds.first().map(|b| b.version.clone());
        let left = left.child(label(
            installed.unwrap_or_else(|| "Horizon not installed".into()),
            12.,
            theme::faint(),
        ));
        let (text, action): (&str, Option<Action>) = if horizon::asset_name().is_none() {
            ("NOT AVAILABLE", None)
        } else if h.job.is_some() {
            ("STARTING", None)
        } else if h.running {
            ("PLAYING", None)
        } else if self.java.sign_in.is_some() {
            ("SIGNING IN", None)
        } else if self.java.accounts.selected().is_none() {
            ("SIGN IN", Some(Launcher::start_sign_in))
        } else if h.builds.is_empty() {
            ("INSTALL", Some(Launcher::play_java))
        } else {
            ("PLAY", Some(Launcher::play_java))
        };
        let play = div()
            .id("java-play")
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
        let message = h.message.as_ref().or(self.java.message.as_ref());
        let right = if let Some(job) = &h.job {
            let step = match job.fraction() {
                Some(f) => format!("{} · {:.0}%", job.step(), f * 100.),
                None => job.step(),
            };
            right
                .child(label(step, 12., theme::muted()).text_right())
                .child(progress_bar(job.fraction(), 220.))
        } else if horizon::asset_name().is_none() {
            right.child(
                label(
                    "Horizon has no build for this computer.",
                    12.,
                    theme::muted(),
                )
                .text_right(),
            )
        } else {
            match message {
                Some(Ok(text)) => {
                    right.child(label(text.clone(), 12., theme::muted()).text_right())
                }
                Some(Err(text)) => right
                    .child(label(text.clone(), 12., theme::error()).text_right())
                    .children(self.crash_link(Edition::Java, cx)),
                None => right
                    .child(label(self.horizon_release_line(), 12., theme::muted()).text_right()),
            }
        };
        div()
            .h(px(112.))
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

    /// The Accounts section of Settings.
    pub fn accounts_settings(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(6.));
        let selected = self.java.accounts.selected.clone();
        for account in &self.java.accounts.accounts {
            let is_selected = selected.as_deref() == Some(account.uuid.as_str());
            let select_uuid = account.uuid.clone();
            let out_uuid = account.uuid.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(4.))
                    .bg(theme::panel())
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(self.head(&account.uuid, 28.))
                            .child(
                                label(account.name.clone(), 14., theme::text())
                                    .font_weight(FontWeight::SEMIBOLD),
                            ),
                    )
                    .child(if is_selected {
                        label("Selected", 12., theme::muted()).into_any_element()
                    } else {
                        button(
                            SharedString::from(format!("account-select-{}", account.uuid)),
                            "Use",
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_account(select_uuid.clone(), cx)
                        }))
                        .into_any_element()
                    })
                    .child(
                        button(
                            SharedString::from(format!("account-out-{}", account.uuid)),
                            "Sign out",
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| this.sign_out(out_uuid.clone(), cx),
                        )),
                    ),
            );
        }
        if self.java.accounts.accounts.is_empty() {
            list = list.child(label("No accounts yet.", 13., theme::muted()));
        }
        let add = if self.java.sign_in.is_some() {
            disabled("Add account").into_any_element()
        } else {
            button("account-add", "Add account")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.start_sign_in(cx)))
                .into_any_element()
        };
        list.child(div().flex().child(add))
    }
}

/// `https://www.microsoft.com/link` → `microsoft.com/link`.
fn display_url(url: &str) -> &str {
    let url = url.strip_prefix("https://").unwrap_or(url);
    url.strip_prefix("www.").unwrap_or(url)
}
