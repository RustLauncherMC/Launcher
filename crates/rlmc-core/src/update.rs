//! RustLauncherMC updating itself from its GitHub releases.
//!
//! 1. The latest release's tag (`vX.Y.Z`) is compared with this build's version.
//! 2. The release's `SHA256SUMS` must be signed by a key in `release-keys.txt` (built in), and
//!    the download must match its checksum. A build with no key built in never updates itself.
//! 3. The new launcher replaces the running one: the AppImage file itself (`$APPIMAGE`), or the
//!    executable. On Windows the running executable can't be overwritten, so it is renamed to
//!    `rustlauncher.exe.old-<pid>` first and deleted on a later start ([`cleanup`]).
//!
//! Installs the launcher can't write to (a system package in `/usr/bin`) and development builds
//! (inside a Cargo `target` folder) are not updated; `RUSTLAUNCHERMC_NO_SELF_UPDATE` turns
//! updates off too.

use crate::download::{self, Progress};
use crate::github::{self, Release};
use crate::signing;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const REPO: &str = "RustLauncherMC/Launcher";
pub const RELEASES_URL: &str = "https://github.com/RustLauncherMC/Launcher/releases";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROJECT: &str = "rustlaunchermc";
const SUMS: &str = "SHA256SUMS";
const SIGNATURE: &str = "SHA256SUMS.sig";
const KEYS: &str = include_str!("../../../release-keys.txt");
const DISABLE_ENV: &str = "RUSTLAUNCHERMC_NO_SELF_UPDATE";

/// Release asset names, shared with `.github/workflows/release.yml`.
pub mod assets {
    pub const APPIMAGE: &str = "RustLauncherMC-x86_64.AppImage";
    pub const LINUX_TAR: &str = "rustlauncher-linux-x86_64.tar.gz";
    pub const WINDOWS_ZIP: &str = "rustlauncher-windows-x64.zip";
}

/// `v1.2.3` or `1.2.3` → (1, 2, 3). Pre-release suffixes are ignored.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim().trim_start_matches('v');
    let core = text.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// The newest release, if it is newer than this build.
pub fn check(agent: &ureq::Agent) -> Result<Option<Release>> {
    let release = github::release(agent, REPO, "latest")?;
    Ok(is_newer(&release.tag_name, VERSION).then_some(release))
}

fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(new), Some(current)) => new > current,
        _ => false,
    }
}

/// How this copy of the launcher would be replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The AppImage file at this path.
    AppImage(PathBuf),
    /// The executable at this path, from a `.tar.gz` (Linux) or `.zip` (Windows).
    Executable(PathBuf),
}

impl Target {
    pub fn path(&self) -> &Path {
        match self {
            Target::AppImage(p) | Target::Executable(p) => p,
        }
    }

    fn asset(&self) -> Option<&'static str> {
        match self {
            Target::AppImage(_) => Some(assets::APPIMAGE),
            Target::Executable(_) if cfg!(all(windows, target_arch = "x86_64")) => {
                Some(assets::WINDOWS_ZIP)
            }
            Target::Executable(_) if cfg!(all(target_os = "linux", target_arch = "x86_64")) => {
                Some(assets::LINUX_TAR)
            }
            Target::Executable(_) => None,
        }
    }
}

/// What can be updated, or why this copy can't update itself.
pub fn target() -> Result<Target, String> {
    if std::env::var_os(DISABLE_ENV).is_some() {
        return Err(format!("Updates are turned off ({DISABLE_ENV})"));
    }
    if signing::parse_keys(KEYS).map_or(true, |k| k.is_empty()) {
        return Err("This build has no release key, so it can't check updates".into());
    }
    let appimage = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file());
    let target = match appimage {
        Some(path) => Target::AppImage(path),
        None => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            if exe
                .ancestors()
                .any(|a| a.file_name().is_some_and(|n| n == "target"))
            {
                return Err("Development builds don't update themselves".into());
            }
            Target::Executable(exe)
        }
    };
    let dir = target.path().parent().ok_or("no folder")?;
    if !writable(dir) {
        return Err(
            "This copy was installed by a package manager. Update it there, or download the \
             new version"
                .into(),
        );
    }
    if target.asset().is_none() {
        return Err("There are no updates for this computer".into());
    }
    Ok(target)
}

fn writable(dir: &Path) -> bool {
    tempfile::tempfile_in(dir).is_ok()
}

/// Downloads `release`, checks its signature and replaces this launcher with it. The new
/// version runs after a restart ([`restart`]).
pub fn install(
    agent: &ureq::Agent,
    release: &Release,
    target: &Target,
    progress: &Progress,
) -> Result<()> {
    let name = target
        .asset()
        .context("there are no updates for this computer")?;
    let url = |file: &str| -> Result<String> {
        Ok(release
            .asset(file)
            .with_context(|| format!("the release has no {file}"))?
            .browser_download_url
            .clone())
    };
    progress.set_step("Checking the update");
    let sums = download::text(agent, &url(SUMS)?)?;
    let signature = download::text(agent, &url(SIGNATURE)?)?;
    verify(&release.tag_name, sums.as_bytes(), &signature)?;
    let sha256 = crate::checksum::expected(&sums, name)?;

    let path = target.path();
    let dir = path.parent().context("no folder")?;
    let stage = tempfile::Builder::new()
        .prefix(".rustlauncher-update.")
        .tempdir_in(dir)?;
    let download = stage.path().join(name);
    progress.set_step(format!("Downloading RustLauncherMC {}", release.tag_name));
    download::verified(agent, &url(name)?, &download, &sha256, progress)?;

    progress.set_step("Installing the update");
    let new = match target {
        Target::AppImage(_) => download,
        Target::Executable(_) => {
            let exe = stage.path().join(exe_name());
            extract_executable(&download, &exe)?;
            exe
        }
    };
    replace(&new, path)
}

fn verify(tag: &str, sums: &[u8], signature: &str) -> Result<()> {
    let keys = signing::parse_keys(KEYS)?;
    if !signing::signed_by_any(&signing::message(PROJECT, tag, sums), signature, &keys) {
        bail!(
            "the update's signature doesn't match RustLauncherMC's release key, so it was not \
             installed"
        );
    }
    Ok(())
}

fn exe_name() -> String {
    format!("rustlauncher{}", std::env::consts::EXE_SUFFIX)
}

/// Takes the launcher's executable out of a release archive (it sits in one top folder).
fn extract_executable(archive: &Path, dest: &Path) -> Result<()> {
    let wanted = exe_name();
    let name = archive.to_string_lossy();
    let out = || fs::File::create(dest);
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_file()
                && entry
                    .enclosed_name()
                    .is_some_and(|p| p.file_name().is_some_and(|n| n == wanted.as_str()))
            {
                std::io::copy(&mut entry, &mut out()?)?;
                return Ok(());
            }
        }
    } else {
        let gz = flate2::read::GzDecoder::new(fs::File::open(archive)?);
        let mut tar = tar::Archive::new(gz);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            if entry.header().entry_type().is_file()
                && path.file_name().is_some_and(|n| n == wanted.as_str())
            {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                fs::write(dest, bytes)?;
                return Ok(());
            }
        }
    }
    bail!("the update has no {wanted} in it")
}

/// Puts `new` in place of `path`, keeping the old file until the new one is there.
fn replace(new: &Path, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(new, fs::Permissions::from_mode(0o755))?;
        // A rename over the running file is atomic, and the running process keeps its copy.
        fs::rename(new, path).context("could not replace the launcher")?;
    }
    #[cfg(windows)]
    {
        // The running .exe can be renamed but not overwritten.
        let old = old_name(path, std::process::id());
        fs::rename(path, &old).context("could not move the running launcher aside")?;
        if let Err(e) = fs::rename(new, path) {
            let _ = fs::rename(&old, path);
            return Err(e).context("could not replace the launcher");
        }
    }
    Ok(())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn old_name(path: &Path, pid: u32) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".old-{pid}"));
    path.with_file_name(name)
}

/// Deletes executables a previous update moved aside (Windows). Call at startup.
pub fn cleanup() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) else {
        return;
    };
    let prefix = format!("{}.old-", name.to_string_lossy());
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Starts the updated launcher and returns; the caller then quits.
pub fn restart(target: &Target) -> Result<()> {
    let mut command = std::process::Command::new(target.path());
    command.args(std::env::args_os().skip(1));
    if matches!(target, Target::AppImage(_)) {
        // The new AppImage sets these itself.
        for name in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
            command.env_remove(name);
        }
    }
    command
        .spawn()
        .context("could not start the updated launcher")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.10.0-beta.1"), Some((0, 10, 0)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("nightly"), None);
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("v0.10.0", "0.9.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn takes_the_executable_out_of_a_tar_gz() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.tar.gz");
        let gz = flate2::write::GzEncoder::new(
            fs::File::create(&archive).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(gz);
        for (name, data) in [
            ("top/README.md", &b"r"[..]),
            (&*format!("top/{}", exe_name()), &b"new"[..]),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, name, data).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        let out = dir.path().join("exe");
        extract_executable(&archive, &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"new");
    }

    #[test]
    fn replaces_a_file_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(exe_name());
        let new = dir.path().join("new");
        fs::write(&path, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        replace(&new, &path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(!new.exists());
    }

    #[test]
    fn the_built_in_release_keys_parse() {
        // An unreadable or empty keys file would turn self-update off for every launcher.
        let keys = signing::parse_keys(KEYS).expect("release-keys.txt is valid");
        assert!(!keys.is_empty(), "release-keys.txt lists no key");
    }

    #[test]
    fn unsigned_updates_are_refused() {
        assert!(verify("v9.9.9", b"sums", "").is_err());
    }

    #[test]
    fn moved_aside_names_carry_the_pid() {
        assert_eq!(
            old_name(Path::new("/a/rustlauncher.exe"), 42),
            PathBuf::from("/a/rustlauncher.exe.old-42")
        );
    }
}
