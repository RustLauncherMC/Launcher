//! Signed-in Microsoft accounts for Java Edition.
//!
//! - Public details (UUID, name) are in `accounts.json` in the config folder.
//! - Secrets (the Microsoft refresh token, the Minecraft token and when it expires, and the
//!   last ownership answer) are only in the system's password storage: Windows Credential
//!   Manager, or the Secret Service (GNOME Keyring, KWallet) on Linux. No plain file can
//!   claim that an account owns the game.

use crate::microsoft::{self, Minecraft, MsTokens, NotOwned, SignInAgain};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SERVICE: &str = "rustlaunchermc";
/// Windows Credential Manager holds at most 2560 bytes per item, so longer secrets are split
/// over `<uuid>`, `<uuid>.1`, `<uuid>.2`, …
const CHUNK: usize = 1200;
/// Renew the Minecraft token when it has less than this left.
const RENEW_BEFORE: u64 = 60 * 60;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Without dashes, as Minecraft's API gives it.
    pub uuid: String,
    pub name: String,
    /// Signed in to Horizon's own launcher rather than here: its sessions come from there
    /// ([`crate::horizon::bridge`]), and no secret of it is stored here.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub via_horizon: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
}

impl Accounts {
    pub fn path() -> PathBuf {
        crate::paths::config_dir().join("accounts.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        let dir = path.parent().expect("accounts path has a parent");
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.persist(&path)?;
        Ok(())
    }

    pub fn selected(&self) -> Option<&Account> {
        let uuid = self.selected.as_deref()?;
        self.accounts.iter().find(|a| a.uuid == uuid)
    }

    /// Adds or updates an account and selects it.
    fn put(&mut self, account: Account) {
        self.selected = Some(account.uuid.clone());
        match self.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
            Some(existing) => *existing = account,
            None => self.accounts.push(account),
        }
    }

    fn remove(&mut self, uuid: &str) {
        self.accounts.retain(|a| a.uuid != uuid);
        if self.selected.as_deref() == Some(uuid) {
            self.selected = self.accounts.first().map(|a| a.uuid.clone());
        }
    }
}

/// What is kept in the password storage for each account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secrets {
    pub refresh_token: String,
    pub mc_token: String,
    pub mc_expires_at: u64,
    #[serde(default)]
    pub xuid: Option<String>,
    /// The last ownership answer, so a player who passed before can still play offline.
    pub owns_java: bool,
}

/// Where secrets are kept. The real one is [`Keyring`]; tests use [`MemoryStore`].
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// The system's password storage.
pub struct Keyring;

impl SecretStore for Keyring {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match keyring::Entry::new(SERVICE, key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_error(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, key)?
            .set_password(value)
            .map_err(keyring_error)
    }

    fn delete(&self, key: &str) -> Result<()> {
        match keyring::Entry::new(SERVICE, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_error(e)),
        }
    }
}

fn keyring_error(e: keyring::Error) -> anyhow::Error {
    anyhow::Error::new(e).context(
        "the system's password storage is not available. On Linux, a Secret Service such as \
         GNOME Keyring or KWallet must be running",
    )
}

/// Secrets in memory, for tests.
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

fn chunk_key(uuid: &str, index: usize) -> String {
    match index {
        0 => uuid.to_string(),
        n => format!("{uuid}.{n}"),
    }
}

pub fn load_secrets(store: &dyn SecretStore, uuid: &str) -> Result<Option<Secrets>> {
    let mut text = String::new();
    for index in 0.. {
        match store.get(&chunk_key(uuid, index))? {
            Some(part) => text.push_str(&part),
            None if index == 0 => return Ok(None),
            None => break,
        }
    }
    Ok(serde_json::from_str(&text).ok())
}

pub fn save_secrets(store: &dyn SecretStore, uuid: &str, secrets: &Secrets) -> Result<()> {
    let text = serde_json::to_string(secrets)?;
    let parts: Vec<&str> = text
        .as_bytes()
        .chunks(CHUNK)
        // JSON from serde is ASCII-safe to split only on char boundaries; tokens are ASCII.
        .map(|c| std::str::from_utf8(c).expect("secrets are ASCII"))
        .collect();
    for (index, part) in parts.iter().enumerate() {
        store.set(&chunk_key(uuid, index), part)?;
    }
    // A shorter secret than before must not pick up the old trailing chunks.
    let mut index = parts.len();
    while store.get(&chunk_key(uuid, index))?.is_some() {
        store.delete(&chunk_key(uuid, index))?;
        index += 1;
    }
    Ok(())
}

fn delete_secrets(store: &dyn SecretStore, uuid: &str) -> Result<()> {
    let mut index = 0;
    while store.get(&chunk_key(uuid, index))?.is_some() {
        store.delete(&chunk_key(uuid, index))?;
        index += 1;
    }
    Ok(())
}

fn secrets_of(ms: &MsTokens, mc: &Minecraft, old_refresh: Option<&str>) -> Result<Secrets> {
    let refresh_token = ms
        .refresh_token
        .clone()
        .or_else(|| old_refresh.map(str::to_string))
        .context("Microsoft sent no refresh token")?;
    Ok(Secrets {
        refresh_token,
        mc_token: mc.access_token.clone(),
        mc_expires_at: mc.expires_at,
        xuid: mc.xuid.clone(),
        owns_java: true,
    })
}

/// Finishes a sign-in: checks the account in Minecraft's services, then saves it.
pub fn add(
    agent: &ureq::Agent,
    store: &dyn SecretStore,
    accounts: &mut Accounts,
    ms: &MsTokens,
) -> Result<Account> {
    let mc = microsoft::minecraft(agent, &ms.access_token)?;
    let secrets = secrets_of(ms, &mc, None)?;
    save_secrets(store, &mc.uuid, &secrets)?;
    let account = Account {
        uuid: mc.uuid,
        name: mc.name,
        via_horizon: false,
    };
    accounts.put(account.clone());
    Ok(account)
}

pub fn sign_out(store: &dyn SecretStore, accounts: &mut Accounts, uuid: &str) -> Result<()> {
    delete_secrets(store, uuid)?;
    accounts.remove(uuid);
    Ok(())
}

/// What the game needs to play online, as Horizon's session JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xuid: Option<String>,
}

/// How a launch may go for an account.
#[derive(Debug)]
pub enum Launch {
    /// Signed in and checked now: play online.
    Online(Session),
    /// Minecraft could not be reached, but this account owned the game last time: play
    /// offline under its name.
    Offline(Account),
    /// An account of Horizon's own launcher, which owns the game: ask that launcher for a
    /// session file right before starting ([`crate::horizon::bridge`]).
    Horizon(Account),
}

/// Adds the accounts of Horizon's own launcher that aren't here yet, and selects the one it
/// selected. Refused when none of them owns Java Edition. Returns how many were added.
pub fn use_horizon_accounts(
    accounts: &mut Accounts,
    list: &crate::horizon::bridge::AccountList,
) -> Result<usize> {
    if list.accounts.is_empty() {
        bail!("No Microsoft account is signed in to Horizon's launcher");
    }
    if !list.owner {
        bail!("No account in Horizon's launcher owns Minecraft: Java Edition");
    }
    let mut added = 0;
    for borrowed in &list.accounts {
        if !accounts.accounts.iter().any(|a| a.uuid == borrowed.uuid) {
            accounts.accounts.push(Account {
                uuid: borrowed.uuid.clone(),
                name: borrowed.name.clone(),
                via_horizon: true,
            });
            added += 1;
        }
    }
    let pick = list
        .accounts
        .iter()
        .find(|a| a.selected)
        .unwrap_or(&list.accounts[0]);
    accounts.selected = Some(pick.uuid.clone());
    Ok(added)
}

/// For an account of Horizon's own launcher: checks it is still signed in there and that one of
/// its accounts owns the game, before anything is downloaded.
pub fn launch_via_horizon(
    horizon: Option<&crate::horizon::bridge::HorizonLauncher>,
    account: &Account,
) -> Result<Launch> {
    let Some(horizon) = horizon else {
        bail!(
            "{} is signed in through Horizon's launcher, which isn't found. Choose it in Settings",
            account.name
        );
    };
    let list = horizon.accounts()?;
    if !list.accounts.iter().any(|a| a.uuid == account.uuid) {
        bail!(
            "{} is no longer signed in to Horizon's launcher",
            account.name
        );
    }
    if !list.owner {
        bail!("{} doesn't own Minecraft: Java Edition", account.name);
    }
    Ok(Launch::Horizon(account.clone()))
}

/// Checks the account before a launch, as Horizon's launcher does: renews the Minecraft token
/// when it is due, and asks again whether the account owns the game. Without a network, the
/// last answer stands.
pub fn launch(agent: &ureq::Agent, store: &dyn SecretStore, account: &Account) -> Result<Launch> {
    let Some(mut secrets) = load_secrets(store, &account.uuid)? else {
        bail!(
            "{} is not signed in on this computer. Please sign in again",
            account.name
        );
    };
    match check(agent, &mut secrets) {
        Ok(()) => {
            save_secrets(store, &account.uuid, &secrets)?;
            if !secrets.owns_java {
                bail!("{} doesn't own Minecraft: Java Edition", account.name);
            }
            Ok(Launch::Online(Session {
                name: account.name.clone(),
                uuid: account.uuid.clone(),
                access_token: secrets.mc_token,
                user_type: "msa",
                xuid: secrets.xuid,
            }))
        }
        Err(e) if e.is::<SignInAgain>() => Err(e),
        Err(e) if is_offline(&e) && secrets.owns_java => Ok(Launch::Offline(account.clone())),
        Err(e) => Err(e),
    }
}

fn check(agent: &ureq::Agent, secrets: &mut Secrets) -> Result<()> {
    if secrets.mc_expires_at < microsoft::now() + RENEW_BEFORE {
        let ms = microsoft::refresh(agent, &secrets.refresh_token)?;
        let mc = match microsoft::minecraft(agent, &ms.access_token) {
            Ok(mc) => mc,
            Err(e) if e.is::<NotOwned>() => {
                secrets.owns_java = false;
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        *secrets = secrets_of(&ms, &mc, Some(&secrets.refresh_token))?;
    } else {
        secrets.owns_java = microsoft::owns_java(agent, &secrets.mc_token)?;
    }
    Ok(())
}

/// Whether an error means the services could not be reached at all.
fn is_offline(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause
            .downcast_ref::<ureq::Error>()
            .is_some_and(|e| matches!(e, ureq::Error::Transport(_)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets(refresh: &str) -> Secrets {
        Secrets {
            refresh_token: refresh.into(),
            mc_token: "mc".into(),
            mc_expires_at: 1,
            xuid: None,
            owns_java: true,
        }
    }

    #[test]
    fn long_secrets_are_split_and_rejoined() {
        let store = MemoryStore::default();
        let long = secrets(&"r".repeat(5000));
        save_secrets(&store, "u", &long).unwrap();
        assert!(store.get("u.4").unwrap().is_some());
        assert_eq!(load_secrets(&store, "u").unwrap(), Some(long));

        // A shorter secret replaces every old chunk.
        let short = secrets("short");
        save_secrets(&store, "u", &short).unwrap();
        assert!(store.get("u.1").unwrap().is_none());
        assert_eq!(load_secrets(&store, "u").unwrap(), Some(short));

        delete_secrets(&store, "u").unwrap();
        assert_eq!(load_secrets(&store, "u").unwrap(), None);
    }

    #[test]
    fn accounts_select_and_remove() {
        let store = MemoryStore::default();
        let mut accounts = Accounts::default();
        let a = Account {
            uuid: "a".into(),
            name: "Alex".into(),
            ..Default::default()
        };
        let b = Account {
            uuid: "b".into(),
            name: "Steve".into(),
            ..Default::default()
        };
        accounts.put(a.clone());
        accounts.put(b.clone());
        assert_eq!(accounts.selected(), Some(&b));
        accounts.put(Account {
            uuid: "a".into(),
            name: "Alex2".into(),
            ..Default::default()
        });
        assert_eq!(accounts.accounts.len(), 2);
        assert_eq!(accounts.selected().unwrap().name, "Alex2");

        save_secrets(&store, "a", &secrets("r")).unwrap();
        sign_out(&store, &mut accounts, "a").unwrap();
        assert_eq!(accounts.selected(), Some(&b));
        assert_eq!(load_secrets(&store, "a").unwrap(), None);
    }

    #[test]
    fn the_session_json_matches_horizons_launch_api() {
        let session = Session {
            name: "Alex".into(),
            uuid: "069a79f444e94726a5befca90e38aaf5".into(),
            access_token: "t".into(),
            user_type: "msa",
            xuid: None,
        };
        assert_eq!(
            serde_json::to_string(&session).unwrap(),
            r#"{"name":"Alex","uuid":"069a79f444e94726a5befca90e38aaf5","access_token":"t","user_type":"msa"}"#
        );
    }

    #[test]
    fn horizon_accounts_are_added_once_and_need_an_owner() {
        use crate::horizon::bridge::{AccountList, BorrowedAccount};
        let borrowed = |uuid: &str, selected| BorrowedAccount {
            name: format!("P{uuid}"),
            uuid: uuid.into(),
            selected,
        };
        let mut accounts = Accounts::default();
        accounts.put(Account {
            uuid: "a".into(),
            name: "Mine".into(),
            ..Default::default()
        });
        let list = AccountList {
            accounts: vec![borrowed("a", false), borrowed("b", true)],
            owner: true,
        };
        assert_eq!(use_horizon_accounts(&mut accounts, &list).unwrap(), 1);
        assert_eq!(accounts.selected().unwrap().uuid, "b");
        assert!(accounts.selected().unwrap().via_horizon);
        // The account signed in here stays as it is.
        assert!(!accounts.accounts[0].via_horizon);
        assert_eq!(use_horizon_accounts(&mut accounts, &list).unwrap(), 0);

        let no_owner = AccountList {
            owner: false,
            ..list
        };
        assert!(use_horizon_accounts(&mut accounts, &no_owner).is_err());
        let empty = AccountList {
            accounts: vec![],
            owner: true,
        };
        assert!(use_horizon_accounts(&mut accounts, &empty).is_err());
    }

    #[test]
    fn a_horizon_account_without_horizons_launcher_is_explained() {
        let account = Account {
            uuid: "a".into(),
            name: "Alex".into(),
            via_horizon: true,
        };
        let e = launch_via_horizon(None, &account).unwrap_err();
        assert!(e.to_string().contains("Choose it in Settings"));
        // Its stored form says where it comes from; an account signed in here doesn't.
        assert!(serde_json::to_string(&account)
            .unwrap()
            .contains("via_horizon"));
        let mine = Account {
            via_horizon: false,
            ..account
        };
        assert!(!serde_json::to_string(&mine)
            .unwrap()
            .contains("via_horizon"));
    }

    #[test]
    fn launching_a_forgotten_account_asks_to_sign_in() {
        let store = MemoryStore::default();
        let account = Account {
            uuid: "a".into(),
            name: "Alex".into(),
            ..Default::default()
        };
        let e = launch(&crate::http(), &store, &account).unwrap_err();
        assert!(e.to_string().contains("sign in again"));
    }
}
