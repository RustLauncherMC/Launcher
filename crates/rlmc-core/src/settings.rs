//! The launcher's settings, saved as `settings.json` in [`crate::paths::config_dir`].

use crate::cinnabar::Channel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The editions in the sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Java,
    #[default]
    Bedrock,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The edition shown when the launcher opens.
    pub edition: Edition,
    /// Which Cinnabar releases to install.
    pub cinnabar_channel: Channel,
}

impl Settings {
    pub fn path() -> PathBuf {
        crate::paths::config_dir().join("settings.json")
    }

    /// The saved settings, or the defaults when there are none or the file is unreadable.
    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Saves atomically, so a crash never leaves a half-written file.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        let dir = path.parent().expect("settings path has a parent");
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.persist(&path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"edition":"java"}"#).unwrap();
        assert_eq!(s.edition, Edition::Java);
        assert_eq!(s.cinnabar_channel, Channel::Beta);
    }

    #[test]
    fn round_trips() {
        let s = Settings {
            edition: Edition::Bedrock,
            cinnabar_channel: Channel::Nightly,
        };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }
}
