//! Colours and sizes, close to the official launcher's dark theme.

use gpui::{rgb, Rgba};

pub const FONT: &str = "Noto Sans";

pub fn window_bg() -> Rgba {
    rgb(0x262626)
}
pub fn sidebar_bg() -> Rgba {
    rgb(0x1b1b1b)
}
pub fn sidebar_hover() -> Rgba {
    rgb(0x2a2a2a)
}
pub fn sidebar_selected() -> Rgba {
    rgb(0x333333)
}
pub fn bar_bg() -> Rgba {
    rgb(0x1f1f1f)
}
pub fn panel() -> Rgba {
    rgb(0x313131)
}
pub fn panel_hover() -> Rgba {
    rgb(0x3b3b3b)
}
pub fn border() -> Rgba {
    rgb(0x3d3d3d)
}
pub fn text() -> Rgba {
    rgb(0xffffff)
}
pub fn muted() -> Rgba {
    rgb(0xb0b0b0)
}
pub fn faint() -> Rgba {
    rgb(0x7a7a7a)
}
pub fn error() -> Rgba {
    rgb(0xff7b72)
}

/// The Play button, in the official launcher's green.
pub fn play() -> Rgba {
    rgb(0x3c8527)
}
pub fn play_hover() -> Rgba {
    rgb(0x4a9e31)
}
pub fn play_disabled() -> Rgba {
    rgb(0x2f4a28)
}

/// Each edition's colour, from the logo: Horizon purple and Cinnabar red.
pub fn java() -> Rgba {
    rgb(0x3b2f7e)
}
pub fn bedrock() -> Rgba {
    rgb(0xb22222)
}
pub fn grass() -> Rgba {
    rgb(0x6db33f)
}
