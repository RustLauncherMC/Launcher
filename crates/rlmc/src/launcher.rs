//! The window: sidebar, edition header with tabs, and the selected page.

use crate::bedrock::Bedrock;
use crate::horizon::Horizon;
use crate::installations::InstallationsUi;
use crate::java::Java;
use crate::theme;
use crate::updates::Updates;
use crate::widgets::{button, edition_icon, heading, label};
use gpui::{
    div, img, linear_color_stop, linear_gradient, prelude::*, px, ClickEvent, Context, Div,
    FontWeight, Image, ImageFormat, Rgba, SharedString, Window,
};
use rlmc_core::settings::{Edition, Settings};
use std::sync::Arc;

pub const LOGO: &[u8] = include_bytes!("../../../assets/logo-256.png");
const REPO_URL: &str = "https://github.com/RustLauncherMC/Launcher";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Edition(Edition),
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Play,
    Installations,
    PatchNotes,
}

pub struct Launcher {
    pub settings: Settings,
    pub page: Page,
    pub tab: Tab,
    pub agent: rlmc_core::ureq::Agent,
    pub logo: Arc<Image>,
    pub bedrock: Bedrock,
    pub java: Java,
    pub horizon: Horizon,
    pub updates: Updates,
    pub inst: InstallationsUi,
}

impl Launcher {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        let mut launcher = Launcher {
            page: Page::Edition(settings.edition),
            settings,
            tab: Tab::Play,
            agent: rlmc_core::http(),
            logo: Arc::new(Image::from_bytes(ImageFormat::Png, LOGO.to_vec())),
            bedrock: Bedrock::default(),
            java: Java::default(),
            horizon: Horizon::default(),
            updates: Updates::Checking,
            inst: InstallationsUi::load(),
        };
        launcher.find_bedrock(cx);
        launcher.fetch_bedrock_release(cx);
        launcher.fetch_horizon_release(cx);
        rlmc_core::update::cleanup();
        launcher.check_for_updates(cx);
        launcher.load_heads(cx);
        launcher.java.horizon_launcher =
            rlmc_core::horizon::bridge::find(launcher.settings.horizon_launcher.as_deref());
        launcher
    }

    pub fn save_settings(&self) {
        if let Err(e) = self.settings.save() {
            eprintln!("RustLauncherMC: could not save settings: {e:#}");
        }
    }

    fn select(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.page = page;
        self.tab = Tab::Play;
        if let Page::Edition(edition) = page {
            self.settings.edition = edition;
            self.save_settings();
        }
        cx.notify();
    }

    // Sidebar

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(250.))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::sidebar_bg())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(16.))
                    .py(px(18.))
                    .child(img(self.logo.clone()).size(px(36.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(heading("RustLauncherMC", 15.))
                            .child(label("Fan-made launcher", 11., theme::faint())),
                    ),
            )
            .child(div().h(px(1.)).mx(px(16.)).mb(px(10.)).bg(theme::border()))
            .child(self.sidebar_item(
                "nav-java",
                Page::Edition(Edition::Java),
                edition_icon(theme::java()),
                "Java Edition",
                "Horizon",
                cx,
            ))
            .child(self.sidebar_item(
                "nav-bedrock",
                Page::Edition(Edition::Bedrock),
                edition_icon(theme::bedrock()),
                "Bedrock Edition",
                "Cinnabar",
                cx,
            ))
            .child(div().flex_1())
            .children(self.update_banner(cx))
            .child(
                self.sidebar_item(
                    "nav-settings",
                    Page::Settings,
                    div()
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(label("⚙", 18., theme::muted())),
                    "Settings",
                    "Channels, folders, about",
                    cx,
                ),
            )
            .child(div().h(px(12.)))
    }

    fn sidebar_item(
        &self,
        id: &'static str,
        page: Page,
        icon: Div,
        title: &'static str,
        subtitle: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.page == page;
        div()
            .id(id)
            .mx(px(8.))
            .my(px(2.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(4.))
            .flex()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .when(selected, |d| d.bg(theme::sidebar_selected()))
            .when(!selected, |d| d.hover(|s| s.bg(theme::sidebar_hover())))
            .child(icon)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(label(title, 14., theme::text()).font_weight(if selected {
                        FontWeight::BOLD
                    } else {
                        FontWeight::SEMIBOLD
                    }))
                    .child(label(subtitle, 11., theme::muted())),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(page, cx)))
    }

    // Header

    pub fn header(
        &self,
        title: &'static str,
        subtitle: &'static str,
        tabs: &[(Tab, &'static str)],
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().gap(px(28.)).px(px(32.));
        for &(tab, name) in tabs {
            let selected = self.tab == tab;
            row = row.child(
                div()
                    .id(SharedString::from(format!("tab-{name}")))
                    .pb(px(10.))
                    .border_b(px(3.))
                    .border_color(if selected {
                        theme::text()
                    } else {
                        Rgba::default()
                    })
                    .cursor_pointer()
                    .child(
                        label(
                            name.to_uppercase(),
                            13.,
                            if selected {
                                theme::text()
                            } else {
                                theme::muted()
                            },
                        )
                        .font_weight(FontWeight::BOLD),
                    )
                    .hover(|s| s.text_color(theme::text()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    })),
            );
        }
        div()
            .flex()
            .flex_col()
            .bg(theme::bar_bg())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(12.))
                    .px(px(32.))
                    .pt(px(20.))
                    .pb(px(14.))
                    .child(heading(title, 24.))
                    .child(label(subtitle, 14., theme::muted())),
            )
            .child(row)
    }

    // Pages

    fn settings_page(&mut self, cx: &mut Context<Self>) -> Div {
        let channel = self.settings.cinnabar_channel;
        let mut channels = div().flex().gap(px(8.));
        for option in rlmc_core::cinnabar::Channel::ALL {
            let selected = option == channel;
            channels = channels.child(
                button(
                    SharedString::from(format!("settings-channel-{}", option.label())),
                    option.label(),
                )
                .when(selected, |b| {
                    b.bg(theme::play()).border_color(theme::play())
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_bedrock_channel(option, cx)
                })),
            );
        }
        let config = rlmc_core::paths::config_dir();
        let data = rlmc_core::cinnabar::data_dir();
        let section = |title: &'static str| heading(title, 16.).mt(px(18.));
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(32.))
                    .pt(px(20.))
                    .pb(px(14.))
                    .bg(theme::bar_bg())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(heading("SETTINGS", 24.)),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .overflow_x_hidden()
                    .px(px(32.))
                    .py(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(section("Java Edition accounts"))
                    .child(label(
                        "Microsoft accounts for Horizon. Sign-in tokens are kept in your \
                         system's password storage, never in a plain file.",
                        13.,
                        theme::muted(),
                    ))
                    .child(self.accounts_settings(cx))
                    .child(section("Bedrock Edition (Cinnabar)"))
                    .child(label(
                        "Which Cinnabar releases to install. Nightly is rebuilt every day. \
                         Once installed, Cinnabar updates itself on the channel it was built for.",
                        13.,
                        theme::muted(),
                    ))
                    .child(channels)
                    .child(section("Folders"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                button("settings-open-config", "Launcher settings").on_click(
                                    move |_, _, cx| {
                                        let _ = std::fs::create_dir_all(&config);
                                        cx.open_with_system(&config);
                                    },
                                ),
                            )
                            .child(match data.filter(|d| d.exists()) {
                                Some(dir) => button("settings-open-cinnabar", "Cinnabar game data")
                                    .on_click(move |_, _, cx| cx.open_with_system(&dir))
                                    .into_any_element(),
                                None => crate::widgets::disabled("Cinnabar game data")
                                    .into_any_element(),
                            }),
                    )
                    .child(section("Updates"))
                    .child(self.updates_settings(cx))
                    .child(section("About"))
                    .child(label(
                        format!("RustLauncherMC {}", env!("CARGO_PKG_VERSION")),
                        13.,
                        theme::text(),
                    ))
                    .child(label(
                        "A fan project. Not an official Minecraft product, and not approved by or \
                         associated with Mojang or Microsoft, or with the Horizon or Cinnabar \
                         projects. Minecraft is a trademark of Microsoft Corporation.",
                        13.,
                        theme::muted(),
                    ))
                    .child(label(
                        "Cinnabar is downloaded from its official releases and is licensed under \
                         PolyForm Noncommercial 1.0.0. RustLauncherMC is MIT-licensed and \
                         noncommercial. Built with GPUI (Apache-2.0) and Noto Sans (OFL).",
                        13.,
                        theme::muted(),
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .mt(px(4.))
                            .child(
                                button("settings-repo", "Source code")
                                    .on_click(|_, _, cx| cx.open_url(REPO_URL)),
                            )
                            .child(
                                button("settings-cinnabar-license", "Cinnabar's license").on_click(
                                    |_, _, cx| cx.open_url(rlmc_core::cinnabar::LICENSE_URL),
                                ),
                            ),
                    ),
            )
    }
}

/// The large area under the header: the edition's colour fading into the window, with the logo.
pub fn hero(color: Rgba, logo: Arc<Image>) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .p(px(32.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(color, 0.),
            linear_color_stop(theme::window_bg(), 1.),
        ))
        .child(img(logo).size(px(150.)))
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.page {
            Page::Edition(Edition::Java) => self.java_page(window, cx),
            Page::Edition(Edition::Bedrock) => self.bedrock_page(window, cx),
            Page::Settings => self.settings_page(cx),
        };
        div()
            .size_full()
            .flex()
            .font_family(theme::FONT)
            .bg(theme::window_bg())
            .text_color(theme::text())
            .child(self.sidebar(cx))
            // Without a zero minimum width, the page would widen to fit its longest line
            // instead of wrapping it.
            .child(page.min_w(px(0.)))
    }
}
