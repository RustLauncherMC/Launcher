//! Cinnabar on Linux, in the same layout as Cinnabar's own `install.sh`
//! (`packaging/install.sh.in` upstream):
//!
//! ```text
//! $XDG_DATA_HOME/cinnabar/app/
//!     <tag>-<first 12 hex of the AppImage's SHA-256>/   the extracted AppImage (an AppDir)
//!         AppRun, cinnabar.png, usr/bin/cinnabar, ..., .cinnabar-complete
//!     current      the active build's folder name
//!     previous     the one before
//!     .lock        held while installing
//! ```
//!
//! Sharing that layout means an install made by `install.sh` is found here, and the game's own
//! updater recognises installs made here (it replaces an extracted AppDir in place). The
//! launcher does not write `install.sh`'s `~/.local/bin/cinnabar` wrapper or menu entry: the
//! launcher is the way in.

use super::{valid_tag, Install};
use crate::download::{self, Progress};
use crate::github::{Asset, Release};
use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const ASSET: Option<&str> = if cfg!(target_arch = "x86_64") {
    Some("Cinnabar-x86_64.AppImage")
} else {
    None
};

const CURRENT: &str = "current";
const PREVIOUS: &str = "previous";
const MARKER: &str = ".cinnabar-complete";

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
}

pub fn data_dir() -> Option<PathBuf> {
    data_home().map(|d| d.join("cinnabar"))
}

fn app_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("app"))
}

pub fn find() -> Option<Install> {
    find_in(&app_dir()?)
}

fn find_in(app_dir: &Path) -> Option<Install> {
    let build = read_pointer(app_dir, CURRENT)?;
    let folder = app_dir.join(build);
    let executable = folder.join("AppRun");
    is_executable(&executable).then_some(Install { executable, folder })
}

/// A pointer file's first line, if it names a plain folder.
fn read_pointer(app_dir: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(app_dir.join(name)).ok()?;
    let line = text.lines().next()?.trim_end_matches('\r');
    (!line.is_empty() && !line.contains('/') && line != "." && line != "..")
        .then(|| line.to_string())
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn complete(build: &Path) -> bool {
    is_executable(&build.join("AppRun"))
        && build.join("cinnabar.png").is_file()
        && build.join(MARKER).is_file()
}

pub fn install(
    agent: &ureq::Agent,
    release: &Release,
    asset: &Asset,
    sha256: &str,
    progress: &Progress,
) -> Result<Install> {
    let app_dir = app_dir().context("could not find your home folder")?;
    install_in(
        &app_dir,
        release,
        progress,
        |image| {
            progress.set_step(format!("Downloading Cinnabar {}", release.title()));
            download::verified(agent, &asset.browser_download_url, image, sha256, progress)
        },
        sha256,
    )
}

/// The install itself, with the download passed in so tests can supply a file.
fn install_in(
    app_dir: &Path,
    release: &Release,
    progress: &Progress,
    download: impl FnOnce(&Path) -> Result<()>,
    sha256: &str,
) -> Result<Install> {
    if !valid_tag(&release.tag_name) {
        bail!("unexpected release tag {:?}", release.tag_name);
    }
    fs::create_dir_all(app_dir).with_context(|| format!("create {}", app_dir.display()))?;
    // One install at a time, together with Cinnabar's own installer, which uses the same lock.
    let lock = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(app_dir.join(".lock"))?;
    progress.set_step("Waiting for another Cinnabar install to finish");
    lock.lock()?;

    let build = format!("{}-{}", release.tag_name, &sha256[..12]);
    let target = app_dir.join(&build);
    let stage = tempfile::Builder::new()
        .prefix(".install.")
        .tempdir_in(app_dir)?;
    if !complete(&target) {
        let image = stage.path().join("Cinnabar.AppImage");
        download(&image)?;
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755))?;
        progress.set_step("Unpacking Cinnabar");
        let status = Command::new(&image)
            .arg("--appimage-extract")
            .current_dir(stage.path())
            .env_remove("APPIMAGE")
            .env_remove("APPDIR")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("could not unpack the AppImage")?;
        let extracted = stage.path().join("squashfs-root");
        if !status.success() || !is_executable(&extracted.join("AppRun")) {
            bail!("could not unpack the verified AppImage; your existing install was kept");
        }
        if !extracted.join("cinnabar.png").is_file() {
            bail!("the AppImage is missing its icon, so it does not look like a Cinnabar release");
        }
        fs::write(extracted.join(MARKER), b"")?;
        if target.exists() {
            fs::rename(&target, stage.path().join("damaged"))?;
        }
        fs::rename(&extracted, &target)?;
    }

    progress.set_step("Finishing");
    let mut previous = read_pointer(app_dir, CURRENT).unwrap_or_default();
    if previous == build {
        previous = read_pointer(app_dir, PREVIOUS).unwrap_or_default();
    }
    write_atomic(app_dir, PREVIOUS, &format!("{previous}\n"))?;
    write_atomic(app_dir, CURRENT, &format!("{build}\n"))?;
    prune(app_dir, &build, &previous);
    drop(stage);
    drop(lock);
    find_in(app_dir).context("the install finished, but the game could not be found")
}

fn write_atomic(dir: &Path, name: &str, text: &str) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    file.persist(dir.join(name))?;
    Ok(())
}

/// Removes builds other than the current and previous ones, as `install.sh` does.
fn prune(app_dir: &Path, keep: &str, previous: &str) {
    let Ok(entries) = fs::read_dir(app_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == keep || name == previous || !is_build_name(&name) {
            continue;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// `<tag>-<12 lowercase hex>`.
fn is_build_name(name: &str) -> bool {
    name.rsplit_once('-').is_some_and(|(tag, hash)| {
        !tag.is_empty()
            && hash.len() == 12
            && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

pub fn adjust(_: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn release(tag: &str) -> Release {
        serde_json::from_value(serde_json::json!({ "tag_name": tag })).unwrap()
    }

    /// A stand-in AppImage: a script that "extracts" a minimal AppDir like the real one.
    fn fake_appimage(path: &Path) -> Result<()> {
        fs::write(
            path,
            "#!/bin/sh\n[ \"$1\" = --appimage-extract ] || exit 1\n\
             mkdir -p squashfs-root/usr/bin\n\
             printf '#!/bin/sh\\necho ok\\n' > squashfs-root/AppRun\n\
             chmod 755 squashfs-root/AppRun\n\
             : > squashfs-root/cinnabar.png\n",
        )?;
        Ok(())
    }

    #[test]
    fn installs_in_install_sh_layout_and_tracks_previous() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("cinnabar/app");
        let progress = Progress::default();

        let first = install_in(&app, &release("v1.0.0"), &progress, fake_appimage, SHA).unwrap();
        assert_eq!(first.folder, app.join("v1.0.0-0123456789ab"));
        assert!(complete(&first.folder));
        assert_eq!(
            read_pointer(&app, CURRENT).as_deref(),
            Some("v1.0.0-0123456789ab")
        );
        assert_eq!(find_in(&app), Some(first.clone()));

        let sha2 = "fedcba9876543210".repeat(4);
        let second = install_in(&app, &release("v1.1.0"), &progress, fake_appimage, &sha2).unwrap();
        assert_eq!(
            read_pointer(&app, CURRENT).as_deref(),
            Some("v1.1.0-fedcba987654")
        );
        assert_eq!(
            read_pointer(&app, PREVIOUS).as_deref(),
            Some("v1.0.0-0123456789ab")
        );
        assert!(first.folder.exists(), "the previous build is kept");
        assert_eq!(find_in(&app), Some(second));

        let sha3 = "aaaaaaaaaaaa".repeat(5) + "aaaa";
        install_in(&app, &release("v1.2.0"), &progress, fake_appimage, &sha3).unwrap();
        assert!(
            !first.folder.exists(),
            "builds older than previous are pruned"
        );
    }

    #[test]
    fn reinstalling_the_same_build_skips_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let progress = Progress::default();
        install_in(&app, &release("v1.0.0"), &progress, fake_appimage, SHA).unwrap();
        let again = install_in(
            &app,
            &release("v1.0.0"),
            &progress,
            |_| bail!("downloaded"),
            SHA,
        );
        assert!(again.is_ok());
    }

    #[test]
    fn a_failed_download_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let progress = Progress::default();
        assert!(install_in(
            &app,
            &release("v1.0.0"),
            &progress,
            |_| bail!("offline"),
            SHA
        )
        .is_err());
        assert_eq!(find_in(&app), None);
    }

    #[test]
    fn rejects_unsafe_tags_and_pointers() {
        let dir = tempfile::tempdir().unwrap();
        let progress = Progress::default();
        assert!(install_in(dir.path(), &release("../x"), &progress, fake_appimage, SHA).is_err());
        fs::write(dir.path().join(CURRENT), "../../etc\n").unwrap();
        assert_eq!(read_pointer(dir.path(), CURRENT), None);
    }

    #[test]
    fn recognises_build_folder_names() {
        assert!(is_build_name("v1.0.0-0123456789ab"));
        assert!(is_build_name("nightly-0123456789ab"));
        assert!(!is_build_name("current"));
        assert!(!is_build_name("v1.0.0-0123456789AB"));
        assert!(!is_build_name(".install.abc"));
    }
}
