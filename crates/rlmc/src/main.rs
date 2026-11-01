//! RustLauncherMC: one launcher for Minecraft rewritten in Rust.
//!
//! The window follows the official launcher's layout: editions in a sidebar on the left, the
//! selected edition's tabs at the top, and a bar at the bottom with the version and Play.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod bedrock;
mod horizon;
mod java;
mod launcher;
mod theme;
mod updates;
mod widgets;

use gpui::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use std::borrow::Cow;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf")),
                Cow::Borrowed(include_bytes!(
                    "../../../assets/fonts/NotoSans-SemiBold.ttf"
                )),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf")),
            ])
            .expect("bundled fonts");
        let bounds = Bounds::centered(None, size(px(1100.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(580.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("RustLauncherMC".into()),
                    ..Default::default()
                }),
                app_id: Some("rustlaunchermc".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| launcher::Launcher::new(window, cx)),
        )
        .expect("could not open the launcher window");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
}
