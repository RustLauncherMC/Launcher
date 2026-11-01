//! Signing in through Horizon's own launcher.
//!
//! When a player already has Horizon's launcher (`horizon-launcher`) signed in, RustLauncherMC can
//! use those accounts instead of its own Microsoft sign-in. Two of its commands do the work
//! (`docs/launcher-commands.md` upstream; a "preview" API, so anyone may use it):
//!
//! - `horizon-launcher account list --json`: the accounts, and whether one owns Java Edition.
//! - `horizon-launcher account session <uuid> --json`: renews that account through Horizon's own
//!   sign-in app, checks again that it owns the game, and writes a read-once session file:
//!   `{"session_file": "<path>"}`, or `{"authentication": {...}}` when it can't.
//!
//! RustLauncherMC never reads the tokens: it passes the file to `horizon-client --session-file`,
//! and the game deletes it once read. A session file older than two minutes is refused by the
//! game, so ask for it only right before starting.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Horizon's launcher, found or chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HorizonLauncher {
    pub exe: PathBuf,
}

/// An account signed in to Horizon's launcher.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BorrowedAccount {
    pub name: String,
    pub uuid: String,
    #[serde(default)]
    pub selected: bool,
}

/// What `account list` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountList {
    pub accounts: Vec<BorrowedAccount>,
    /// Whether one of them owns Minecraft: Java Edition (Horizon's launcher checked it).
    pub owner: bool,
}

fn exe_name() -> String {
    format!("horizon-launcher{}", std::env::consts::EXE_SUFFIX)
}

/// Horizon's launcher: the one chosen in Settings, else one on `PATH`, else (Linux) the one its
/// application-menu entry points at.
pub fn find(chosen: Option<&Path>) -> Option<HorizonLauncher> {
    if let Some(exe) = chosen.filter(|p| p.is_file()) {
        return Some(HorizonLauncher {
            exe: exe.to_path_buf(),
        });
    }
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(exe_name()))
            .find(|p| p.is_file())
    });
    on_path
        .or_else(from_menu_entry)
        .map(|exe| HorizonLauncher { exe })
}

/// Linux: Horizon's launcher writes `applications/horizon-launcher.desktop`, pointing at itself.
fn from_menu_entry() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))?;
    let text = std::fs::read_to_string(data.join("applications/horizon-launcher.desktop")).ok()?;
    exe_from_desktop_entry(&text).filter(|p| p.is_file())
}

/// The program a desktop entry starts: `TryExec` (a plain path), else the first word of `Exec`.
fn exe_from_desktop_entry(text: &str) -> Option<PathBuf> {
    let mut in_entry = false;
    let mut exec = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(path) = line.strip_prefix("TryExec=") {
            return Some(PathBuf::from(path));
        }
        if let Some(value) = line.strip_prefix("Exec=") {
            exec = first_word(value);
        }
    }
    exec.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// The first word of an `Exec` value, with desktop-entry quoting undone.
fn first_word(value: &str) -> Option<String> {
    let value = value.trim();
    let Some(rest) = value.strip_prefix('"') else {
        return value.split_whitespace().next().map(str::to_string);
    };
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

impl HorizonLauncher {
    fn run(&self, args: &[&str]) -> Result<Value> {
        let mut command = Command::new(&self.exe);
        super::launch::quiet(&mut command);
        let output = command
            .args(args)
            .arg("--json")
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("could not run {}", self.exe.display()))?;
        let reply: Value = serde_json::from_slice(&output.stdout)
            .ok()
            .unwrap_or(Value::Null);
        if reply.is_null() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.lines().last().unwrap_or("").trim();
            bail!("Horizon's launcher didn't answer ({detail})");
        }
        Ok(reply)
    }

    /// The accounts signed in to Horizon's launcher.
    pub fn accounts(&self) -> Result<AccountList> {
        parse_accounts(&self.run(&["account", "list"])?)
    }

    /// A fresh, read-once session file for `uuid`, for `horizon-client --session-file`.
    pub fn session_file(&self, uuid: &str) -> Result<PathBuf> {
        if uuid.is_empty() || !uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            bail!("not a player UUID");
        }
        parse_session(&self.run(&["account", "session", uuid])?)
    }
}

fn parse_accounts(reply: &Value) -> Result<AccountList> {
    let accounts = reply["accounts"]
        .as_array()
        .context("Horizon's launcher listed no accounts")?
        .iter()
        .filter_map(|a| serde_json::from_value::<BorrowedAccount>(a.clone()).ok())
        .map(|mut a| {
            a.uuid = a.uuid.replace('-', "").to_ascii_lowercase();
            a
        })
        .filter(|a| a.uuid.len() == 32 && a.uuid.bytes().all(|b| b.is_ascii_hexdigit()))
        .collect();
    Ok(AccountList {
        accounts,
        owner: reply["owner"].as_bool().unwrap_or(false),
    })
}

fn parse_session(reply: &Value) -> Result<PathBuf> {
    if let Some(path) = reply["session_file"].as_str() {
        let path = PathBuf::from(path);
        let plausible = path.is_absolute()
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".json"));
        if !plausible || !path.is_file() {
            bail!("Horizon's launcher gave an unexpected session file");
        }
        return Ok(path);
    }
    let auth = &reply["authentication"];
    let message = auth["message"]
        .as_str()
        .or_else(|| auth["label"].as_str())
        .unwrap_or("it couldn't renew the account's sign-in");
    bail!("Horizon's launcher: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_account_list() {
        let list = parse_accounts(&json!({
            "accounts": [
                {"name": "Alex", "uuid": "069A79F4-44E9-4726-A5BE-FCA90E38AAF5", "selected": true,
                 "authentication": {"state": "ready"}},
                {"name": "bad", "uuid": "nope"}
            ],
            "offline": [], "owner": true, "sign_in_configured": true
        }))
        .unwrap();
        assert_eq!(list.accounts.len(), 1);
        assert_eq!(list.accounts[0].uuid, "069a79f444e94726a5befca90e38aaf5");
        assert!(list.accounts[0].selected && list.owner);
        assert!(parse_accounts(&json!({})).is_err());
    }

    #[test]
    fn reads_session_replies() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session-ab.json");
        std::fs::write(&file, "{}").unwrap();
        let ok = json!({"session_file": file});
        assert_eq!(parse_session(&ok).unwrap(), file);
        let missing = json!({"session_file": dir.path().join("gone.json")});
        assert!(parse_session(&missing).is_err());
        let relative = json!({"session_file": "session.json"});
        assert!(parse_session(&relative).is_err());
        let refused = json!({"authentication": {"state": "expired", "message": "Sign in again"}});
        assert_eq!(
            parse_session(&refused).unwrap_err().to_string(),
            "Horizon's launcher: Sign in again"
        );
    }

    #[test]
    fn finds_the_program_in_a_desktop_entry() {
        let entry = "[Desktop Entry]\nExec=\"/home/a b/Horizon \\\"x\\\"/horizon-launcher\"\nTryExec=/home/a b/Horizon \"x\"/horizon-launcher\n";
        assert_eq!(
            exe_from_desktop_entry(entry),
            Some(PathBuf::from("/home/a b/Horizon \"x\"/horizon-launcher"))
        );
        let no_try = "[Desktop Action play]\nExec=/a\n[Desktop Entry]\nExec=\"/opt/b/horizon-launcher\" %U\n";
        assert_eq!(
            exe_from_desktop_entry(no_try),
            Some(PathBuf::from("/opt/b/horizon-launcher"))
        );
        assert_eq!(
            exe_from_desktop_entry("[Desktop Entry]\nExec=flatpak run x\n"),
            None
        );
    }

    /// A stand-in `horizon-launcher` script that answers both commands like the real one.
    #[cfg(unix)]
    #[test]
    fn talks_to_a_horizon_launcher() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("horizon-launcher");
        let session = dir.path().join("session-0011.json");
        std::fs::write(
            &exe,
            format!(
                "#!/bin/sh\n\
                 case \"$1 $2\" in\n\
                 'account list') echo '{{\"accounts\":[{{\"name\":\"Alex\",\"uuid\":\"069a79f444e94726a5befca90e38aaf5\",\"selected\":true}}],\"owner\":true}}' ;;\n\
                 'account session') [ \"$3\" = 069a79f444e94726a5befca90e38aaf5 ] || {{ echo '{{\"authentication\":{{\"message\":\"Unknown account\"}}}}'; exit 1; }}\n\
                   echo '{{}}' > '{s}'; echo '{{\"session_file\":\"{s}\"}}' ;;\n\
                 *) echo nope >&2; exit 2 ;;\n\
                 esac\n",
                s = session.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let launcher = find(Some(&exe)).unwrap();
        let list = launcher.accounts().unwrap();
        assert_eq!(list.accounts[0].name, "Alex");
        assert_eq!(
            launcher
                .session_file("069a79f444e94726a5befca90e38aaf5")
                .unwrap(),
            session
        );
        let err = launcher
            .session_file("00000000000000000000000000000000")
            .unwrap_err();
        assert_eq!(err.to_string(), "Horizon's launcher: Unknown account");
        assert!(launcher.session_file("--help").is_err());
    }
}
