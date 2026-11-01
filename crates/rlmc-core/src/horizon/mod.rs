//! Minecraft: Java Edition through [Horizon](https://github.com/horizon-rust/horizon).
//!
//! Unlike Cinnabar, Horizon leaves the launcher to do the work, as its own launcher does:
//!
//! 1. **Install** the nightly build: download it, check it against Horizon's signed
//!    `SHA256SUMS` ([`sign`]), and unpack it into `versions/<version>/`.
//! 2. **Prepare** the game's resources from the player's own Minecraft files with the build's
//!    `horizon-prepare` ([`prepare`]). The files come from the official launcher's folder, or
//!    are downloaded from Mojang only for a signed-in owner ([`mojang`]).
//! 3. **Launch** `horizon-client` through Horizon's Launch API, handing the signed-in account
//!    over on standard input ([`launch`]), or as a session file from Horizon's own launcher
//!    ([`bridge`]).
//!
//! Everything lives under `<data>/horizon/`:
//!
//! ```text
//! versions/<version>/   unpacked builds (the newest two are kept), each with `prepared/`
//! minecraft/            Minecraft downloaded from Mojang, in the official layout
//! server-jars/<mc>.jar  the server jar the preparer needs once
//! runtime/              Mojang's Java runtime, when the system has no matching Java
//! home/                 the game folder: saves, options, logs and crash reports
//! ```

pub mod bridge;
pub mod launch;
pub mod mojang;
pub mod prepare;
pub mod sign;

mod extract;

use crate::download::{self, Progress};
use crate::github::{self, Release};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const REPO: &str = "horizon-rust/horizon";
pub const RELEASES_URL: &str = "https://github.com/horizon-rust/horizon/releases";
const INSTALLED: &str = ".installed";
/// Builds kept installed, newest first, so the one before stays a rollback.
const KEEP: usize = 2;

/// The release file for this computer, or `None` where Horizon publishes none we support.
pub fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("horizon-linux-x86_64.tar.gz"),
        ("windows", "x86_64") => Some("horizon-windows-x64.zip"),
        _ => None,
    }
}

fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

pub fn root() -> PathBuf {
    crate::paths::data_dir().join("horizon")
}

fn versions_dir() -> PathBuf {
    root().join("versions")
}

/// The game folder: saves, options, logs and crash reports.
pub fn home_dir() -> PathBuf {
    root().join("home")
}

/// Where Minecraft is downloaded to when the official launcher's folder has no usable copy.
pub fn minecraft_dir() -> PathBuf {
    root().join("minecraft")
}

/// The newest nightly build.
pub fn latest(agent: &ureq::Agent) -> Result<Release> {
    github::release(agent, REPO, &format!("tags/{}", sign::NIGHTLY))
}

/// The name a release installs as, such as `nightly-2026-10-28-0c02067`.
pub fn version_of(release: &Release) -> String {
    sign::nightly_version(
        release.title(),
        release.published_at.as_deref().unwrap_or(""),
    )
}

/// An installed build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    pub version: String,
    pub dir: PathBuf,
}

impl Build {
    pub fn client(&self) -> PathBuf {
        self.dir.join(exe("horizon-client"))
    }

    pub fn preparer(&self) -> PathBuf {
        self.dir.join(exe("horizon-prepare"))
    }

    /// The build's prepared resources.
    pub fn prepared_dir(&self) -> PathBuf {
        self.dir.join("prepared")
    }

    /// The Minecraft version and files this build was made for.
    pub fn pins(&self) -> Result<Pins> {
        Pins::read(&self.dir.join("research-lock.json"))
    }
}

/// Installed builds, newest first.
pub fn installed() -> Vec<Build> {
    installed_in(&versions_dir())
}

fn installed_in(dir: &Path) -> Vec<Build> {
    let mut builds: Vec<Build> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(INSTALLED).is_file())
        .map(|e| Build {
            version: e.file_name().to_string_lossy().into_owned(),
            dir: e.path(),
        })
        .collect();
    // `nightly-<date>-<commit>` sorts by date.
    builds.sort_by(|a, b| b.version.cmp(&a.version));
    builds
}

/// Downloads `release` for this computer, checks its signature and installs it.
pub fn install(agent: &ureq::Agent, release: &Release, progress: &Progress) -> Result<Build> {
    let name = asset_name().context("Horizon has no build for this computer")?;
    let version = version_of(release);
    let dir = versions_dir();
    install_in(
        &dir,
        &version,
        progress,
        |archive| {
            let url = |file: &str| -> Result<String> {
                Ok(release
                    .asset(file)
                    .with_context(|| format!("the Horizon release has no {file}"))?
                    .browser_download_url
                    .clone())
            };
            progress.set_step("Checking the Horizon release");
            let sums = download::text(agent, &url(sign::SUMS)?)?;
            let signature = download::text(agent, &url(sign::SIGNATURE)?)?;
            sign::verify(&release.tag_name, sums.as_bytes(), &signature)?;
            let sha256 = crate::checksum::expected(&sums, name)?;
            progress.set_step(format!("Downloading Horizon {version}"));
            download::verified(agent, &url(name)?, archive, &sha256, progress)
        },
        name,
    )
}

fn install_in(
    dir: &Path,
    version: &str,
    progress: &Progress,
    download: impl FnOnce(&Path) -> Result<()>,
    asset: &str,
) -> Result<Build> {
    if version.is_empty()
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        bail!("unexpected Horizon version {version:?}");
    }
    let target = dir.join(version);
    if target.join(INSTALLED).is_file() {
        return Ok(Build {
            version: version.to_string(),
            dir: target,
        });
    }
    fs::create_dir_all(dir)?;
    let stage = tempfile::Builder::new()
        .prefix(".install.")
        .tempdir_in(dir)?;
    let archive = stage.path().join(asset);
    download(&archive)?;
    progress.set_step("Unpacking Horizon");
    let unpacked = stage.path().join("build");
    extract::unpack(&archive, &unpacked)?;
    if !unpacked.join(exe("horizon-client")).is_file() {
        bail!("the Horizon download has no game in it");
    }
    fs::write(unpacked.join(INSTALLED), b"")?;
    if target.exists() {
        // A half-finished earlier install.
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&unpacked, &target)?;
    drop(stage);
    prune(dir, version);
    Ok(Build {
        version: version.to_string(),
        dir: target,
    })
}

/// Keeps the newest [`KEEP`] builds and the one just installed.
fn prune(dir: &Path, keep: &str) {
    for build in installed_in(dir).iter().skip(KEEP) {
        if build.version != keep {
            let _ = fs::remove_dir_all(&build.dir);
        }
    }
}

/// What a build was made for, from its `research-lock.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pins {
    /// The Minecraft version, such as `26.3`.
    pub version: String,
    /// The version file's accepted SHA-1s (Mojang republished some versions).
    pub manifest_sha1s: Vec<String>,
    pub client_sha1: String,
    pub server_sha1: String,
    pub java_major: u32,
}

impl Pins {
    pub fn read(path: &Path) -> Result<Pins> {
        let lock: Value = serde_json::from_slice(
            &fs::read(path).with_context(|| format!("read {}", path.display()))?,
        )
        .context("the Horizon build's research-lock.json is unreadable")?;
        Self::from_lock(&lock)
    }

    fn from_lock(lock: &Value) -> Result<Pins> {
        let mc = &lock["minecraft"];
        let text = |key: &str| -> Result<String> {
            mc[key]
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("research-lock.json has no minecraft.{key}"))
        };
        let mut manifest_sha1s = vec![text("manifest_sha1")?];
        if let Some(more) = mc["accepted_manifest_sha1"].as_array() {
            manifest_sha1s.extend(more.iter().filter_map(|v| v.as_str()).map(str::to_string));
        }
        Ok(Pins {
            version: text("version")?,
            manifest_sha1s,
            client_sha1: text("client_sha1")?,
            server_sha1: text("server_sha1")?,
            java_major: mc["java_major"]
                .as_u64()
                .context("research-lock.json has no minecraft.java_major")?
                as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_pins() {
        let pins = Pins::from_lock(&json!({"minecraft": {
            "version": "26.3", "manifest_sha1": "a", "accepted_manifest_sha1": ["b", "c"],
            "client_sha1": "cl", "server_sha1": "sv", "java_major": 25
        }}))
        .unwrap();
        assert_eq!(pins.version, "26.3");
        assert_eq!(pins.manifest_sha1s, ["a", "b", "c"]);
        assert_eq!(pins.java_major, 25);
        assert!(Pins::from_lock(&json!({})).is_err());
    }

    fn fake_build(archive: &Path) -> Result<()> {
        // A .tar.gz with one top folder holding the client.
        let gz =
            flate2::write::GzEncoder::new(fs::File::create(archive)?, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(
            &mut header,
            format!("top/{}", exe("horizon-client")),
            &b"x"[..],
        )?;
        tar.into_inner()?.finish()?;
        Ok(())
    }

    #[test]
    fn installs_lists_and_prunes_builds() {
        let dir = tempfile::tempdir().unwrap();
        let progress = Progress::default();
        let asset = "b.tar.gz";
        for v in [
            "nightly-2026-10-01-a",
            "nightly-2026-10-02-b",
            "nightly-2026-10-03-c",
        ] {
            let build = install_in(dir.path(), v, &progress, fake_build, asset).unwrap();
            assert!(build.client().is_file());
        }
        let versions: Vec<_> = installed_in(dir.path())
            .into_iter()
            .map(|b| b.version)
            .collect();
        assert_eq!(versions, ["nightly-2026-10-03-c", "nightly-2026-10-02-b"]);

        // An installed build is not downloaded again.
        let again = install_in(
            dir.path(),
            "nightly-2026-10-03-c",
            &progress,
            |_| bail!("downloaded"),
            asset,
        );
        assert!(again.is_ok());
    }

    #[test]
    fn a_failed_download_installs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let progress = Progress::default();
        let result = install_in(
            dir.path(),
            "nightly-x",
            &progress,
            |_| bail!("offline"),
            "b.tar.gz",
        );
        assert!(result.is_err());
        assert!(installed_in(dir.path()).is_empty());
        assert!(install_in(dir.path(), "../x", &progress, fake_build, "b.tar.gz").is_err());
    }
}
