//! Preparing Horizon's resources with the build's own `horizon-prepare`, which turns the
//! player's Minecraft files into what `horizon-client --resources` reads (Launch API,
//! "Preparing"). It downloads nothing and checks every file against the build's pins.

use super::{mojang, Build};
use crate::download::Progress;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Whether the build's resources are complete and current (`horizon-prepare --check`).
pub fn is_prepared(build: &Build) -> bool {
    command(build)
        .arg("--check")
        .arg("--output")
        .arg(build.prepared_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Makes the build's resources ready, getting Minecraft's files first if needed. Downloads from
/// Mojang happen only here, so the caller must have checked that the signed-in account owns
/// Java Edition.
pub fn ensure(agent: &ureq::Agent, build: &Build, progress: &Progress) -> Result<()> {
    progress.set_step("Checking Horizon's game files");
    if is_prepared(build) {
        return Ok(());
    }
    let pins = build.pins()?;
    let own = super::minecraft_dir();
    let official = mojang::official_dir().filter(|d| mojang::has_game(d, &pins));
    let minecraft = if mojang::has_game(&own, &pins) {
        own.clone()
    } else if let Some(official) = official {
        official
    } else {
        mojang::download_game(agent, &pins, &own, progress)?;
        own.clone()
    };
    let result = prepare_from(agent, build, &pins, &minecraft, progress);
    match result {
        // The official launcher's folder may lack some libraries or sounds: download a full
        // copy and try once more.
        Err(_) if minecraft != own => {
            mojang::download_game(agent, &pins, &own, progress)?;
            prepare_from(agent, build, &pins, &own, progress)
        }
        other => other,
    }
}

fn prepare_from(
    agent: &ureq::Agent,
    build: &Build,
    pins: &super::Pins,
    minecraft: &Path,
    progress: &Progress,
) -> Result<()> {
    let manifest = mojang::version_json(minecraft, pins)?;
    let server = super::root()
        .join("server-jars")
        .join(format!("{}.jar", pins.version));
    mojang::download_server(agent, &manifest, pins, &server, progress)?;
    let java = mojang::java_home(
        agent,
        &manifest,
        pins.java_major,
        &super::root().join("runtime"),
        progress,
    )?;
    run(build, minecraft, &server, java.as_deref(), progress)
}

fn command(build: &Build) -> Command {
    let mut command = Command::new(build.preparer());
    // The preparer keeps its scratch files in `.local/` beside itself.
    command.current_dir(&build.dir);
    super::launch::quiet(&mut command);
    command
}

/// Runs `horizon-prepare --prepare`, following its `--progress json` lines.
pub fn run(
    build: &Build,
    minecraft: &Path,
    server_jar: &Path,
    java_home: Option<&Path>,
    progress: &Progress,
) -> Result<()> {
    progress.set_step("Preparing Horizon's game files. This takes a minute or two");
    let mut command = command(build);
    command
        .arg("--prepare")
        .arg("--output")
        .arg(build.prepared_dir())
        .arg("--complete")
        .args(["--progress", "json"])
        .arg("--server-jar")
        .arg(server_jar)
        .arg("--minecraft-dir")
        .arg(minecraft);
    if let Some(java) = java_home {
        command.arg("--java-home").arg(java);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not start Horizon's preparer")?;

    // Keep the end of stderr for the error message, without letting the pipe fill up.
    let stderr = child.stderr.take().expect("piped");
    let tail = Arc::new(Mutex::new(Vec::<String>::new()));
    let reader = {
        let tail = tail.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut tail = tail.lock().unwrap();
                tail.push(line);
                if tail.len() > 20 {
                    tail.remove(0);
                }
            }
        })
    };

    let mut error = None;
    let stdout = child.stdout.take().expect("piped");
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        match parse_line(&line) {
            Event::Fraction(f) => progress.set_fraction(f),
            Event::Error(e) => error = Some(e),
            Event::Message(m) => eprintln!("horizon-prepare: {m}"),
            Event::Other => {}
        }
    }
    let status = child.wait()?;
    let _ = reader.join();
    if status.success() {
        return Ok(());
    }
    let detail = error
        .or_else(|| tail.lock().unwrap().last().cloned())
        .unwrap_or_else(|| format!("exit code {:?}", status.code()));
    bail!("Horizon could not prepare its game files: {detail}")
}

#[derive(Debug, PartialEq)]
enum Event {
    Message(String),
    Fraction(f64),
    Error(String),
    Other,
}

/// One `--progress json` line. Unknown keys and lines are ignored, as the Launch API asks.
fn parse_line(line: &str) -> Event {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Event::Other;
    };
    if let Some(e) = value["error"].as_str() {
        Event::Error(e.to_string())
    } else if let Some(f) = value["fraction"].as_f64() {
        Event::Fraction(f)
    } else if let Some(m) = value["message"].as_str() {
        Event::Message(m.to_string())
    } else {
        Event::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_progress_lines() {
        assert_eq!(parse_line(r#"{"fraction": 0.5}"#), Event::Fraction(0.5));
        assert_eq!(
            parse_line(r#"{"message": "hi"}"#),
            Event::Message("hi".into())
        );
        assert_eq!(
            parse_line(r#"{"error": "no java"}"#),
            Event::Error("no java".into())
        );
        assert_eq!(parse_line(r#"{"new_key": 1}"#), Event::Other);
        assert_eq!(parse_line("plain text"), Event::Other);
    }
}
