//! Platforms the launcher does not support yet (it targets Windows and Linux).

use crate::github::{Asset, Release};
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

pub const ASSET: Option<&str> = None;

pub fn find() -> Option<super::Install> {
    None
}

pub fn install(
    _: &ureq::Agent,
    _: &Release,
    _: &Asset,
    _: &str,
    _: &crate::download::Progress,
) -> Result<super::Install> {
    bail!("RustLauncherMC supports Windows and Linux only")
}

pub fn adjust(_: &mut Command) {}

pub fn data_dir() -> Option<PathBuf> {
    None
}
