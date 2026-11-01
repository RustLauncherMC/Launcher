//! Installations: named profiles per edition, like the official launcher's Installations tab.
//! Each has its own worlds, settings and logs, so a player can keep a survival world apart from
//! a test world.
//!
//! Every edition has a built-in **Default** installation that can't be removed:
//!
//! | | Default | Others |
//! | --- | --- | --- |
//! | Java (Horizon) | `<data>/horizon/home` | `<data>/horizon/instances/<id>`, as `--game-dir` and `--home` |
//! | Bedrock (Cinnabar) | Cinnabar's own data folder | `<data>/cinnabar/<id>`, as `CINNABAR_USER_ROOT` |
//!
//! A Bedrock installation with its own folder also has its own Cinnabar sign-in, because
//! Cinnabar keeps accounts in its data folder.
//!
//! The list is `installations.json` in the config folder. Removing an installation keeps its
//! folder, so no world is lost by accident.

use crate::settings::Edition;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_ID: &str = "default";
const MAX_NAME: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    /// A short folder-safe id. `default` is the built-in one.
    pub id: String,
    pub name: String,
    pub edition: Edition,
    /// Java only: open the game fullscreen.
    #[serde(default)]
    pub fullscreen: bool,
}

impl Installation {
    pub fn default_for(edition: Edition) -> Self {
        Installation {
            id: DEFAULT_ID.into(),
            name: "Default".into(),
            edition,
            fullscreen: false,
        }
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_ID
    }

    /// Java: the game folder. Bedrock: the data root, or `None` for Cinnabar's own folder.
    pub fn folder(&self) -> Option<PathBuf> {
        match (self.edition, self.is_default()) {
            (Edition::Java, true) => Some(crate::horizon::home_dir()),
            (Edition::Java, false) => Some(crate::horizon::root().join("instances").join(&self.id)),
            (Edition::Bedrock, true) => None,
            (Edition::Bedrock, false) => {
                Some(crate::paths::data_dir().join("cinnabar").join(&self.id))
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Installations {
    /// The installations a player added; the defaults are not stored.
    pub custom: Vec<Installation>,
    pub selected_java: Option<String>,
    pub selected_bedrock: Option<String>,
}

impl Installations {
    pub fn path() -> PathBuf {
        crate::paths::config_dir().join("installations.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        let dir = path.parent().expect("installations path has a parent");
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.persist(&path)?;
        Ok(())
    }

    /// The edition's installations, Default first.
    pub fn list(&self, edition: Edition) -> Vec<Installation> {
        std::iter::once(Installation::default_for(edition))
            .chain(self.custom.iter().filter(|i| i.edition == edition).cloned())
            .collect()
    }

    fn selected_id(&self, edition: Edition) -> Option<&str> {
        match edition {
            Edition::Java => self.selected_java.as_deref(),
            Edition::Bedrock => self.selected_bedrock.as_deref(),
        }
    }

    /// The installation Play starts; Default when none is chosen or the chosen one is gone.
    pub fn selected(&self, edition: Edition) -> Installation {
        let list = self.list(edition);
        self.selected_id(edition)
            .and_then(|id| list.iter().find(|i| i.id == id).cloned())
            .unwrap_or_else(|| Installation::default_for(edition))
    }

    pub fn select(&mut self, edition: Edition, id: &str) {
        let id = Some(id.to_string());
        match edition {
            Edition::Java => self.selected_java = id,
            Edition::Bedrock => self.selected_bedrock = id,
        }
    }

    /// Adds an installation named "Installation N" and selects it.
    pub fn add(&mut self, edition: Edition) -> Installation {
        let taken = |name: &str| self.list(edition).iter().any(|i| i.name == name);
        let name = (2..)
            .map(|n| format!("Installation {n}"))
            .find(|n| !taken(n))
            .expect("some name is free");
        let mut n = self.custom.len() + 1;
        let id = loop {
            let id = format!("installation-{n}");
            if !self.custom.iter().any(|i| i.id == id) && !self.folder_taken(edition, &id) {
                break id;
            }
            n += 1;
        };
        let installation = Installation {
            id,
            name,
            edition,
            fullscreen: false,
        };
        self.custom.push(installation.clone());
        self.select(edition, &installation.id);
        installation
    }

    /// An id whose folder already exists belongs to a removed installation: don't reuse it, so
    /// a new installation never starts with someone else's worlds.
    fn folder_taken(&self, edition: Edition, id: &str) -> bool {
        Installation {
            id: id.into(),
            name: String::new(),
            edition,
            fullscreen: false,
        }
        .folder()
        .is_some_and(|f| f.exists())
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            bail!("A name can't be empty");
        }
        if name.chars().count() > MAX_NAME {
            bail!("A name can be at most {MAX_NAME} characters");
        }
        match self.custom.iter_mut().find(|i| i.id == id) {
            Some(installation) => {
                installation.name = name.to_string();
                Ok(())
            }
            None => bail!("The Default installation can't be renamed"),
        }
    }

    pub fn set_fullscreen(&mut self, id: &str, fullscreen: bool) {
        if let Some(i) = self.custom.iter_mut().find(|i| i.id == id) {
            i.fullscreen = fullscreen;
        }
    }

    /// Removes an installation from the list. Its folder stays.
    pub fn remove(&mut self, id: &str) {
        self.custom.retain(|i| i.id != id);
        for selected in [&mut self.selected_java, &mut self.selected_bedrock] {
            if selected.as_deref() == Some(id) {
                *selected = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_comes_first_and_is_selected_by_default() {
        let mut list = Installations::default();
        assert_eq!(list.selected(Edition::Java).id, DEFAULT_ID);
        let added = list.add(Edition::Java);
        assert_eq!(added.name, "Installation 2");
        assert_eq!(list.selected(Edition::Java), added);
        let names: Vec<_> = list
            .list(Edition::Java)
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["Default", "Installation 2"]);
        // Bedrock is separate.
        assert_eq!(list.list(Edition::Bedrock).len(), 1);
        assert_eq!(list.selected(Edition::Bedrock).id, DEFAULT_ID);
    }

    #[test]
    fn rename_and_remove() {
        let mut list = Installations::default();
        let a = list.add(Edition::Bedrock);
        list.rename(&a.id, "  Survival  ").unwrap();
        assert_eq!(list.selected(Edition::Bedrock).name, "Survival");
        assert!(list.rename(&a.id, " ").is_err());
        assert!(list.rename(&a.id, &"x".repeat(41)).is_err());
        assert!(list.rename(DEFAULT_ID, "Mine").is_err());
        let b = list.add(Edition::Bedrock);
        assert_ne!(a.id, b.id);
        // "Installation 2" was renamed, so its name is free again.
        assert_eq!(b.name, "Installation 2");
        list.remove(&b.id);
        assert_eq!(list.selected(Edition::Bedrock).id, DEFAULT_ID);
        assert_eq!(list.list(Edition::Bedrock).len(), 2);
    }

    #[test]
    fn folders_follow_the_edition() {
        let java = Installation::default_for(Edition::Java);
        assert_eq!(java.folder(), Some(crate::horizon::home_dir()));
        assert_eq!(Installation::default_for(Edition::Bedrock).folder(), None);
        let mut list = Installations::default();
        let custom = list.add(Edition::Bedrock);
        assert!(custom
            .folder()
            .unwrap()
            .ends_with(format!("cinnabar/{}", custom.id)));
    }

    #[test]
    fn a_missing_selection_falls_back_to_default() {
        let list = Installations {
            selected_java: Some("gone".into()),
            ..Default::default()
        };
        assert_eq!(list.selected(Edition::Java).id, DEFAULT_ID);
    }
}
