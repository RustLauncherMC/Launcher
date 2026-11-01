//! Cinnabar on Windows: the official per-machine MSI, installed into
//! `Program Files\Cinnabar\` (`packaging/windows/cinnabar.wxs` upstream). The game updates
//! itself there with the same MSI.

use super::Install;
use crate::download::{self, Progress};
use crate::github::{Asset, Release};
use anyhow::{bail, Context, Result};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

pub const ASSET: Option<&str> = if cfg!(target_arch = "x86_64") {
    Some("Cinnabar-x64.msi")
} else {
    None
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn program_files() -> Option<PathBuf> {
    // A 32-bit process sees the x86 folder as ProgramFiles; the MSI installs into the 64-bit one.
    std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
}

pub fn find() -> Option<Install> {
    let folder = program_files()?.join("Cinnabar");
    let executable = folder.join("cinnabar.exe");
    executable
        .is_file()
        .then_some(Install { executable, folder })
}

pub fn install(
    agent: &ureq::Agent,
    release: &Release,
    asset: &Asset,
    sha256: &str,
    progress: &Progress,
) -> Result<Install> {
    if !super::valid_tag(&release.tag_name) {
        bail!("unexpected release tag {:?}", release.tag_name);
    }
    let dir = crate::paths::cache_dir().join("cinnabar");
    std::fs::create_dir_all(&dir)?;
    let msi = dir.join(format!("Cinnabar-{}.msi", release.tag_name));
    progress.set_step(format!("Downloading Cinnabar {}", release.title()));
    download::verified(agent, &asset.browser_download_url, &msi, sha256, progress)?;

    progress.set_step("Installing. Windows may ask for permission");
    let status = Command::new("msiexec")
        .arg("/i")
        .arg(&msi)
        .args(["/passive", "/norestart"])
        .status()
        .context("could not start the Windows Installer")?;
    let _ = std::fs::remove_file(&msi);
    match status.code() {
        // 3010: installed, a restart is needed to finish.
        Some(0) | Some(3010) => {}
        Some(1602) => bail!("the installation was cancelled"),
        Some(code) => bail!("the Windows Installer failed (error {code})"),
        None => bail!("the Windows Installer stopped unexpectedly"),
    }
    find().context("the installer finished, but Cinnabar is not in Program Files")
}

pub fn adjust(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Cinnabar"))
}
