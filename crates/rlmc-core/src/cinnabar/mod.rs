//! Minecraft: Bedrock Edition through [Cinnabar](https://github.com/cinnabar-mc/cinnabar).
//!
//! Cinnabar does most things itself: Microsoft sign-in, account switching, the EULA prompt and
//! vanilla assets, and its own updates. So the launcher only:
//!
//! 1. finds an existing install, or installs the official release in Cinnabar's own layout, so
//!    the game's updater keeps working on it;
//! 2. starts the game with no arguments and watches for it to exit.
//!
//! Cinnabar is licensed under PolyForm Noncommercial 1.0.0. The launcher downloads official
//! releases from GitHub and never bundles or modifies them.

use crate::github::{self, Release};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
use unsupported as platform;

pub const REPO: &str = "cinnabar-mc/cinnabar";
pub const RELEASES_URL: &str = "https://github.com/cinnabar-mc/cinnabar/releases";
pub const LICENSE_URL: &str = "https://polyformproject.org/licenses/noncommercial/1.0.0";
const SUMS: &str = "SHA256SUMS.txt";

/// Cinnabar's release tracks. ("Stable" is reserved upstream and not published yet.)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Tagged `vX.Y.Z` releases, GitHub's "latest".
    #[default]
    Beta,
    /// The rolling `nightly` pre-release, rebuilt daily.
    Nightly,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Beta, Channel::Nightly];

    pub fn label(self) -> &'static str {
        match self {
            Channel::Beta => "Beta",
            Channel::Nightly => "Nightly",
        }
    }

    fn release_path(self) -> &'static str {
        match self {
            Channel::Beta => "latest",
            Channel::Nightly => "tags/nightly",
        }
    }
}

/// The newest release on `channel`.
pub fn latest(agent: &ureq::Agent, channel: Channel) -> Result<Release> {
    github::release(agent, REPO, channel.release_path())
}

/// The release file for this computer, or `None` where Cinnabar publishes none.
pub fn asset_name() -> Option<&'static str> {
    platform::ASSET
}

/// An installed copy of Cinnabar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    /// What to run to start the game.
    pub executable: PathBuf,
    /// The folder the game is installed in.
    pub folder: PathBuf,
}

/// What an installed game reports about itself (`cinnabar --update-build-info`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: String,
    #[serde(default)]
    pub number: Option<u64>,
    #[serde(default)]
    pub channel: Option<String>,
}

impl BuildInfo {
    pub fn describe(&self) -> String {
        match &self.channel {
            Some(channel) => format!("{} ({channel})", self.version),
            None => self.version.clone(),
        }
    }
}

/// The installed game, if there is one: ours, or one made by Cinnabar's own installer.
pub fn find() -> Option<Install> {
    platform::find()
}

/// Downloads `release`, checks it against the release's `SHA256SUMS.txt`, and installs it.
pub fn install(
    agent: &ureq::Agent,
    release: &Release,
    progress: &crate::download::Progress,
) -> Result<Install> {
    let Some(name) = asset_name() else {
        bail!("Cinnabar has no release for this computer");
    };
    let asset = release
        .asset(name)
        .with_context(|| format!("release {} has no {name}", release.tag_name))?;
    let sums = release
        .asset(SUMS)
        .with_context(|| format!("release {} has no {SUMS}", release.tag_name))?;
    progress.set_step("Checking the release");
    let sums = crate::download::text(agent, &sums.browser_download_url)?;
    let sha256 = crate::checksum::expected(&sums, name)?;
    platform::install(agent, release, asset, &sha256, progress)
}

/// Asks the installed game for its version. It answers before opening any window.
pub fn build_info(install: &Install) -> Result<BuildInfo> {
    let output = command(install)
        .arg("--update-build-info")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("could not run the installed game")?;
    if !output.status.success() {
        bail!("the installed game did not report its version");
    }
    serde_json::from_slice(&output.stdout).context("the installed game reported an unknown version")
}

/// Starts the game's home menu, where the player signs in and picks a world or server.
///
/// `user_root` gives the game its own folder for settings, worlds and sign-in
/// (`CINNABAR_USER_ROOT`); `server` (`host:port`) joins that server directly.
pub fn launch(
    install: &Install,
    user_root: Option<&std::path::Path>,
    server: Option<&str>,
) -> Result<Child> {
    let mut command = command(install);
    if let Some(root) = user_root {
        if !root.is_absolute() {
            bail!("an installation's folder must be an absolute path");
        }
        std::fs::create_dir_all(root)?;
        command.env("CINNABAR_USER_ROOT", root);
    }
    if let Some(server) = server {
        command.arg("--address").arg(server);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("could not start Cinnabar")
}

fn command(install: &Install) -> Command {
    let mut command = Command::new(&install.executable);
    command.current_dir(&install.folder);
    // When this launcher runs as an AppImage, its own AppImage variables must not leak into the
    // game's, as Cinnabar's updater also makes sure.
    for name in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        command.env_remove(name);
    }
    platform::adjust(&mut command);
    command
}

/// Where Cinnabar keeps worlds, logs, packs and sign-in (not the program).
pub fn data_dir() -> Option<PathBuf> {
    platform::data_dir()
}

/// Release tags become folder names, so they must be plain.
fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag != "."
        && tag != ".."
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_must_be_plain() {
        assert!(valid_tag("v0.9.0"));
        assert!(valid_tag("nightly"));
        assert!(!valid_tag(""));
        assert!(!valid_tag(".."));
        assert!(!valid_tag("a/b"));
        assert!(!valid_tag("a b"));
    }

    #[test]
    fn reads_build_info_with_and_without_channel() {
        let a: BuildInfo = serde_json::from_str(r#"{"version":"0.9.0","number":42}"#).unwrap();
        assert_eq!(a.describe(), "0.9.0");
        let b: BuildInfo =
            serde_json::from_str(r#"{"version":"0.9.1","number":43,"channel":"nightly"}"#).unwrap();
        assert_eq!(b.describe(), "0.9.1 (nightly)");
    }
}
