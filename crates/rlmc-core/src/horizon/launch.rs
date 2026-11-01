//! Starting `horizon-client` through Horizon's Launch API (`docs/api/launch.md` upstream):
//!
//! 1. Ask the build which options it takes (`--launch-api`); pass only those, since any other
//!    is refused with exit code 2.
//! 2. Start it with the prepared resources and a game folder, handing the signed-in account
//!    over on standard input (`--session-stdin`), never on the command line.
//! 3. Watch the exit code: on 1 or 101 the last stderr line names the crash report.

use super::Build;
use crate::accounts::Session;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const CRASH_LINE: &str = "Horizon: crash report saved to ";

/// The options this build's `horizon-client` takes.
pub fn options(build: &Build) -> Result<Vec<String>> {
    let mut command = Command::new(build.client());
    quiet(&mut command);
    let output = command
        .arg("--launch-api")
        .current_dir(&build.dir)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("could not run Horizon")?;
    if !output.status.success() {
        bail!("this Horizon build has no Launch API, so RustLauncherMC can't start it");
    }
    parse_options(&output.stdout)
}

fn parse_options(reply: &[u8]) -> Result<Vec<String>> {
    let reply: Value =
        serde_json::from_slice(reply).context("Horizon's Launch API reply is unreadable")?;
    Ok(reply["options"]
        .as_array()
        .context("Horizon's Launch API lists no options")?
        .iter()
        .filter_map(|o| o["name"].as_str().map(str::to_string))
        .collect())
}

/// How to start the game, besides the account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartOptions {
    pub fullscreen: bool,
    /// `host` or `host:port`: join this server at start.
    pub server: Option<String>,
}

/// The arguments for a start, from the options the build takes.
fn arguments(
    options: &[String],
    resources: &Path,
    home: &Path,
    online: bool,
    start: &StartOptions,
) -> Result<Vec<String>> {
    let has = |name: &str| options.iter().any(|o| o == name);
    if !has("--resources") {
        bail!("this Horizon build doesn't take --resources, so RustLauncherMC can't start it");
    }
    let mut args = vec!["--resources".into(), resources.display().to_string()];
    for name in ["--game-dir", "--home"] {
        if has(name) {
            args.push(name.into());
            args.push(home.display().to_string());
        }
    }
    if online {
        if !has("--session-stdin") {
            bail!("this Horizon build can't receive a sign-in safely (no --session-stdin)");
        }
        args.push("--session-stdin".into());
    }
    if start.fullscreen && has("--fullscreen") {
        args.push("--fullscreen".into());
    }
    if let Some(server) = &start.server {
        if !has("--quickPlayMultiplayer") {
            bail!("this Horizon build can't join a server at start");
        }
        args.push("--quickPlayMultiplayer".into());
        args.push(server.clone());
    }
    Ok(args)
}

/// A running game.
pub struct Game {
    pub child: Child,
    /// The last lines Horizon wrote to stderr.
    pub stderr_tail: Arc<Mutex<Vec<String>>>,
}

impl Game {
    /// The crash report Horizon named, after a crash.
    pub fn crash_report(&self) -> Option<String> {
        crash_report(&self.stderr_tail.lock().unwrap())
    }
}

fn crash_report(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .rev()
        .find_map(|l| l.strip_prefix(CRASH_LINE))
        .map(|p| p.trim().to_string())
}

/// Starts the game. With `session`, the player can join online-mode servers; without, the game
/// runs offline.
pub fn start(
    build: &Build,
    session: Option<&Session>,
    home: &Path,
    start: &StartOptions,
) -> Result<Game> {
    let options = options(build)?;
    let args = arguments(
        &options,
        &build.prepared_dir(),
        home,
        session.is_some(),
        start,
    )?;
    std::fs::create_dir_all(home)?;
    let mut command = Command::new(build.client());
    quiet(&mut command);
    let mut child = command
        .args(&args)
        .current_dir(home)
        .stdin(if session.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not start Horizon")?;
    if let Some(session) = session {
        let mut stdin = child.stdin.take().expect("piped");
        let json = serde_json::to_vec(session)?;
        // Closing stdin (by dropping it) tells the game the session is complete.
        if let Err(e) = stdin.write_all(&json) {
            let _ = child.kill();
            return Err(e).context("could not hand the account to Horizon");
        }
    }
    let stderr = child.stderr.take().expect("piped");
    let stderr_tail = Arc::new(Mutex::new(Vec::new()));
    {
        let tail = stderr_tail.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut tail = tail.lock().unwrap();
                tail.push(line);
                if tail.len() > 50 {
                    tail.remove(0);
                }
            }
        });
    }
    Ok(Game { child, stderr_tail })
}

/// No console window on Windows, and none of this launcher's AppImage variables in the child.
pub(super) fn quiet(command: &mut Command) {
    for name in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        command.env_remove(name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = r#"{"launch_api":"0.1.0","options":[
        {"name":"--resources","value":"DIR"},{"name":"--game-dir","value":"DIR"},
        {"name":"--home","value":"DIR"},{"name":"--session-stdin"}]}"#;

    #[test]
    fn passes_only_options_the_build_takes() {
        let options = parse_options(REPLY.as_bytes()).unwrap();
        let args = arguments(
            &options,
            Path::new("/p"),
            Path::new("/h"),
            true,
            &StartOptions::default(),
        )
        .unwrap();
        assert_eq!(
            args,
            [
                "--resources",
                "/p",
                "--game-dir",
                "/h",
                "--home",
                "/h",
                "--session-stdin"
            ]
        );
        let offline = arguments(
            &options,
            Path::new("/p"),
            Path::new("/h"),
            false,
            &StartOptions::default(),
        )
        .unwrap();
        assert!(!offline.contains(&"--session-stdin".to_string()));

        let old = vec!["--resources".to_string()];
        assert_eq!(
            arguments(
                &old,
                Path::new("/p"),
                Path::new("/h"),
                false,
                &StartOptions::default()
            )
            .unwrap(),
            ["--resources", "/p"]
        );
        assert!(arguments(
            &old,
            Path::new("/p"),
            Path::new("/h"),
            true,
            &StartOptions::default()
        )
        .is_err());
        assert!(arguments(
            &[],
            Path::new("/p"),
            Path::new("/h"),
            false,
            &StartOptions::default()
        )
        .is_err());

        let join = StartOptions {
            fullscreen: true,
            server: Some("play.example.net".into()),
        };
        let mut all = options.clone();
        all.extend([
            "--fullscreen".to_string(),
            "--quickPlayMultiplayer".to_string(),
        ]);
        let args = arguments(&all, Path::new("/p"), Path::new("/h"), false, &join).unwrap();
        assert!(args.ends_with(&[
            "--fullscreen".to_string(),
            "--quickPlayMultiplayer".to_string(),
            "play.example.net".to_string()
        ]));
        assert!(arguments(&options, Path::new("/p"), Path::new("/h"), false, &join).is_err());
    }

    #[test]
    fn finds_the_crash_report() {
        let lines = vec![
            "warning".to_string(),
            "Horizon: crash report saved to /h/crash-reports/crash-1-client.txt".to_string(),
        ];
        assert_eq!(
            crash_report(&lines).as_deref(),
            Some("/h/crash-reports/crash-1-client.txt")
        );
        assert_eq!(crash_report(&["x".to_string()]), None);
    }
}
