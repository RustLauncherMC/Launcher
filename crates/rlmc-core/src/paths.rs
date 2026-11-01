//! Where the launcher keeps its own files. These never overlap with the games' folders.
//!
//! | | Windows | Linux |
//! | --- | --- | --- |
//! | config | `%APPDATA%\RustLauncherMC` | `$XDG_CONFIG_HOME/rustlaunchermc` |
//! | cache (downloads) | `%LOCALAPPDATA%\RustLauncherMC\cache` | `$XDG_CACHE_HOME/rustlaunchermc` |
//!
//! `RUSTLAUNCHERMC_HOME` puts both under one folder instead, for testing and portable use.

use std::path::PathBuf;

const HOME_VAR: &str = "RUSTLAUNCHERMC_HOME";

#[cfg(windows)]
const APP_DIR: &str = "RustLauncherMC";
#[cfg(not(windows))]
const APP_DIR: &str = "rustlaunchermc";

fn home_override() -> Option<PathBuf> {
    std::env::var_os(HOME_VAR)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// The folder for `settings.json`.
pub fn config_dir() -> PathBuf {
    if let Some(home) = home_override() {
        return home.join("config");
    }
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_DIR)
}

/// The folder for downloads that can be thrown away, such as installer files.
pub fn cache_dir() -> PathBuf {
    if let Some(home) = home_override() {
        return home.join("cache");
    }
    #[cfg(windows)]
    let base = dirs::data_local_dir().map(|d| d.join(APP_DIR).join("cache"));
    #[cfg(not(windows))]
    let base = dirs::cache_dir().map(|d| d.join(APP_DIR));
    base.unwrap_or_else(|| std::env::temp_dir().join(APP_DIR))
}
