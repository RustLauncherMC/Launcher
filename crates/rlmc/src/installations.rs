//! Installations in the UI: the picker beside Play, the Installations tab's list, and
//! "Join a server" on the Play tab. Both editions share it.

use crate::launcher::Launcher;
use crate::theme;
use crate::widgets::{button, edition_icon, heading, label, path_label, Field, FieldKey};
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, FontWeight, KeyDownEvent, SharedString, Window,
};
use rlmc_core::installations::{Installation, Installations};
use rlmc_core::servers::Address;
use rlmc_core::settings::Edition;

#[derive(Default)]
pub struct InstallationsUi {
    pub list: Installations,
    /// The edition whose picker is open.
    picker: Option<Edition>,
    /// The installation being renamed, and its name so far.
    rename: Option<(String, Field)>,
    /// "Join a server": the address being typed.
    pub server: Option<Field>,
    pub error: Option<String>,
}

impl InstallationsUi {
    pub fn load() -> Self {
        InstallationsUi {
            list: Installations::load(),
            ..Default::default()
        }
    }
}

fn edition_color(edition: Edition) -> gpui::Rgba {
    match edition {
        Edition::Java => theme::java(),
        Edition::Bedrock => theme::bedrock(),
    }
}

impl Launcher {
    fn save_installations(&mut self) {
        self.inst.error = self.inst.list.save().err().map(crate::widgets::friendly);
    }

    pub fn selected_installation(&self, edition: Edition) -> Installation {
        self.inst.list.selected(edition)
    }

    fn select_installation(&mut self, edition: Edition, id: &str, cx: &mut Context<Self>) {
        self.inst.list.select(edition, id);
        self.inst.picker = None;
        self.save_installations();
        cx.notify();
    }

    fn add_installation(&mut self, edition: Edition, window: &mut Window, cx: &mut Context<Self>) {
        let added = self.inst.list.add(edition);
        self.save_installations();
        self.start_rename(&added, window, cx);
    }

    fn start_rename(
        &mut self,
        installation: &Installation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = Field::new(installation.name.clone(), cx);
        window.focus(&field.focus, cx);
        self.inst.rename = Some((installation.id.clone(), field));
        cx.notify();
    }

    fn rename_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some((id, field)) = &mut self.inst.rename else {
            return;
        };
        match field.key(event, cx) {
            FieldKey::Submit => {
                let (id, name) = (id.clone(), field.text.clone());
                match self.inst.list.rename(&id, &name) {
                    Ok(()) => {
                        self.inst.rename = None;
                        self.save_installations();
                    }
                    Err(e) => self.inst.error = Some(e.to_string()),
                }
            }
            FieldKey::Cancel => {
                self.inst.rename = None;
                self.inst.error = None;
            }
            FieldKey::Edited | FieldKey::Ignored => {}
        }
        cx.notify();
    }

    fn remove_installation(&mut self, id: String, cx: &mut Context<Self>) {
        self.inst.list.remove(&id);
        if self.inst.rename.as_ref().is_some_and(|(r, _)| *r == id) {
            self.inst.rename = None;
        }
        self.save_installations();
        cx.notify();
    }

    fn toggle_fullscreen(&mut self, id: String, cx: &mut Context<Self>) {
        let on = self
            .inst
            .list
            .custom
            .iter()
            .find(|i| i.id == id)
            .is_some_and(|i| i.fullscreen);
        self.inst.list.set_fullscreen(&id, !on);
        self.save_installations();
        cx.notify();
    }

    /// The installation picker in the bottom bar: the selected installation, and a list above it
    /// when open.
    pub fn installation_picker(&self, edition: Edition, cx: &mut Context<Self>) -> Div {
        let selected = self.selected_installation(edition);
        let open = self.inst.picker == Some(edition);
        let mut picker = div().relative().child(
            div()
                .id("installation-picker")
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .py(px(4.))
                .rounded(px(4.))
                .cursor_pointer()
                .bg(if open {
                    theme::panel_hover()
                } else {
                    theme::panel()
                })
                .hover(|s| s.bg(theme::panel_hover()))
                .child(
                    label(selected.name.clone(), 13., theme::text())
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(label(if open { "▴" } else { "▾" }, 11., theme::muted()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.inst.picker = if this.inst.picker == Some(edition) {
                        None
                    } else {
                        Some(edition)
                    };
                    cx.notify();
                })),
        );
        if open {
            let mut menu = div()
                .absolute()
                .bottom(px(34.))
                .left(px(0.))
                .w(px(240.))
                .p(px(4.))
                .rounded(px(6.))
                .bg(theme::sidebar_bg())
                .border_1()
                .border_color(theme::border())
                .flex()
                .flex_col()
                .gap(px(2.));
            for installation in self.inst.list.list(edition) {
                let id = installation.id.clone();
                let is_selected = id == selected.id;
                menu = menu.child(
                    div()
                        .id(SharedString::from(format!("pick-{id}")))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(is_selected, |d| d.bg(theme::sidebar_selected()))
                        .hover(|s| s.bg(theme::sidebar_hover()))
                        .child(label(installation.name.clone(), 13., theme::text()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_installation(edition, &id, cx)
                        })),
                );
            }
            menu = menu.child(
                div()
                    .id("pick-manage")
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::sidebar_hover()))
                    .child(label("Manage installations…", 12., theme::muted()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.inst.picker = None;
                        this.tab = crate::launcher::Tab::Installations;
                        cx.notify();
                    })),
            );
            picker = picker.child(menu);
        }
        picker
    }

    /// The Installations tab's list for an edition.
    pub fn installations_list(
        &self,
        edition: Edition,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.selected_installation(edition).id;
        let mut list = div().flex().flex_col().gap(px(8.));
        for installation in self.inst.list.list(edition) {
            let id = installation.id.clone();
            let renaming = self.inst.rename.as_ref().filter(|(r, _)| *r == id);
            let name: gpui::AnyElement = match renaming {
                Some((_, field)) => field
                    .view(
                        SharedString::from(format!("rename-{id}")),
                        "Name",
                        260.,
                        window,
                    )
                    .on_key_down(
                        cx.listener(|this, e: &KeyDownEvent, _, cx| this.rename_key(e, cx)),
                    )
                    .into_any_element(),
                None => heading(installation.name.clone(), 15.).into_any_element(),
            };
            let folder = installation.folder();
            let where_text = match &folder {
                Some(path) => path.display().to_string(),
                None => "Cinnabar's own folder".to_string(),
            };
            let mut buttons = div().flex().gap(px(6.)).flex_none();
            if id != selected {
                let select_id = id.clone();
                buttons = buttons.child(
                    button(SharedString::from(format!("use-{id}")), "Use").on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.select_installation(edition, &select_id, cx)
                        },
                    )),
                );
            }
            if let Some(path) = folder.clone() {
                buttons = buttons.child(
                    button(SharedString::from(format!("open-{id}")), "Open folder").on_click(
                        move |_, _, cx| {
                            let _ = std::fs::create_dir_all(&path);
                            cx.open_with_system(&path);
                        },
                    ),
                );
            }
            if !installation.is_default() {
                if edition == Edition::Java {
                    let full_id = id.clone();
                    buttons = buttons.child(
                        button(
                            SharedString::from(format!("full-{id}")),
                            if installation.fullscreen {
                                "Fullscreen: on"
                            } else {
                                "Fullscreen: off"
                            },
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.toggle_fullscreen(full_id.clone(), cx)
                            },
                        )),
                    );
                }
                if renaming.is_none() {
                    let rename = installation.clone();
                    buttons = buttons.child(
                        button(SharedString::from(format!("rename-btn-{id}")), "Rename").on_click(
                            cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.start_rename(&rename, window, cx)
                            }),
                        ),
                    );
                }
                let remove_id = id.clone();
                buttons = buttons.child(
                    button(SharedString::from(format!("remove-{id}")), "Remove").on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.remove_installation(remove_id.clone(), cx)
                        }),
                    ),
                );
            }
            list = list.child(
                div()
                    .p(px(12.))
                    .rounded(px(6.))
                    .bg(theme::panel())
                    .when(id == selected, |d| d.border_1().border_color(theme::play()))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(edition_icon(edition_color(edition)).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(name)
                            .child(path_label(where_text)),
                    )
                    .child(buttons),
            );
        }
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(heading("Installations", 18.))
            .child(
                button("installation-new", "New installation").on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.add_installation(edition, window, cx)
                    },
                )),
            );
        let note = match edition {
            Edition::Java => {
                "Each installation has its own worlds, settings, resource packs and logs. \
                              Removing one keeps its folder."
            }
            Edition::Bedrock => {
                "Each installation has its own worlds, settings and Cinnabar sign-in, \
                                 so a new one asks you to accept the Minecraft EULA and sign in \
                                 again. Removing one keeps its folder."
            }
        };
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(header)
            .child(list)
            .child(label(note, 12., theme::faint()));
        if let Some(error) = &self.inst.error {
            section = section.child(label(error.clone(), 12., theme::error()));
        }
        section
    }

    /// "Join a server" on the Play tab: a link, then an address field with Join.
    pub fn join_server(&self, edition: Edition, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(field) = &self.inst.server else {
            return div().child(
                div()
                    .id("join-open")
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(theme::muted())
                    .hover(|s| s.text_color(theme::text()).underline())
                    .child("Join a server…")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let field = Field::new("", cx);
                        window.focus(&field.focus, cx);
                        this.inst.server = Some(field);
                        this.inst.error = None;
                        cx.notify();
                    })),
            );
        };
        let placeholder = match edition {
            Edition::Java => "play.example.net or host:25565",
            Edition::Bedrock => "play.example.net or host:19132",
        };
        let mut row =
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    field
                        .view("join-field", placeholder, 280., window)
                        .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                            let Some(field) = &mut this.inst.server else {
                                return;
                            };
                            match field.key(e, cx) {
                                FieldKey::Submit => this.join(edition, cx),
                                FieldKey::Cancel => this.inst.server = None,
                                _ => {}
                            }
                            cx.notify();
                        })),
                )
                .child(crate::widgets::primary_button("join-go", "Join").on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.join(edition, cx)),
                ))
                .child(button("join-cancel", "Cancel").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.inst.server = None;
                        this.inst.error = None;
                        cx.notify();
                    },
                )));
        if let Some(error) = &self.inst.error {
            row = div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(row)
                .child(label(error.clone(), 12., theme::error()));
        }
        row
    }

    fn join(&mut self, edition: Edition, cx: &mut Context<Self>) {
        let Some(field) = &self.inst.server else {
            return;
        };
        match Address::parse(&field.text) {
            Ok(address) => {
                self.inst.error = None;
                match edition {
                    Edition::Java => self.play_java_with(Some(address.java()), cx),
                    Edition::Bedrock => self.play_bedrock_with(Some(address.bedrock()), cx),
                }
                self.inst.server = None;
            }
            Err(e) => self.inst.error = Some(e.to_string()),
        }
        cx.notify();
    }
}
