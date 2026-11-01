//! The games' logs and crash reports, for the launcher's Logs tab.
//!
//! | | Logs | Crash reports |
//! | --- | --- | --- |
//! | Horizon (each installation's game folder) | `logs/latest.log` | `crash-reports/crash-*-client.txt` |
//! | Cinnabar (its data folder) | `logs/client.log`, `logs/core.log` | `crashes/<time>-<id>/report.json` |
//!
//! Only the end of a long file is read, so a huge log never stalls the window.

use crate::installations::Installation;
use crate::settings::Edition;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How much of a file is read: its last 512 KiB.
pub const MAX_BYTES: u64 = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Log,
    Crash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFile {
    /// What the list shows, such as `latest.log` or `Crash at 2026-11-01 12:30`.
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
    pub modified: Option<SystemTime>,
}

/// The folder an installation's game writes logs under: Horizon's game folder, or Cinnabar's
/// data folder (`<installation>/data` for one with its own folder).
pub fn root(installation: &Installation) -> Option<PathBuf> {
    match installation.edition {
        Edition::Java => installation.folder(),
        Edition::Bedrock => match installation.folder() {
            Some(own) => Some(own.join("data")),
            None => crate::cinnabar::data_dir(),
        },
    }
}

/// The installation's logs, then its crash reports, newest first.
pub fn files(installation: &Installation) -> Vec<LogFile> {
    let Some(root) = root(installation) else {
        return Vec::new();
    };
    match installation.edition {
        Edition::Java => horizon_files(&root),
        Edition::Bedrock => cinnabar_files(&root),
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn log(path: PathBuf) -> Option<LogFile> {
    if !path.is_file() {
        return None;
    }
    Some(LogFile {
        name: path.file_name()?.to_string_lossy().into_owned(),
        modified: modified(&path),
        kind: Kind::Log,
        path,
    })
}

fn newest_first(files: &mut [LogFile]) {
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.name.cmp(&a.name))
    });
}

pub fn horizon_files(home: &Path) -> Vec<LogFile> {
    let mut out: Vec<LogFile> = log(home.join("logs/latest.log")).into_iter().collect();
    let mut crashes: Vec<LogFile> = fs::read_dir(home.join("crash-reports"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("crash-") && n.ends_with(".txt")
                })
        })
        .map(|path| LogFile {
            name: crash_name(&path),
            modified: modified(&path),
            kind: Kind::Crash,
            path,
        })
        .collect();
    newest_first(&mut crashes);
    out.extend(crashes);
    out
}

pub fn cinnabar_files(data: &Path) -> Vec<LogFile> {
    let logs = data.join("logs");
    let mut out: Vec<LogFile> = ["client.log", "core.log"]
        .iter()
        .filter_map(|n| log(logs.join(n)))
        .collect();
    let mut crashes: Vec<LogFile> = fs::read_dir(data.join("crashes"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("report.json"))
        .filter(|p| p.is_file())
        .map(|path| LogFile {
            name: crash_name(&path),
            modified: modified(&path),
            kind: Kind::Crash,
            path,
        })
        .collect();
    newest_first(&mut crashes);
    out.extend(crashes);
    out
}

/// "Crash at 2026-11-01 12:30" from the file's time, else the file or folder name.
fn crash_name(path: &Path) -> String {
    if let Some(time) = modified(path) {
        if let Some(text) = format_time(time) {
            return format!("Crash at {text}");
        }
    }
    let named = if path.file_name().is_some_and(|n| n == "report.json") {
        path.parent().and_then(Path::file_name)
    } else {
        path.file_name()
    };
    format!(
        "Crash {}",
        named
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    )
}

/// `YYYY-MM-DD HH:MM` in UTC, without a date library.
pub fn format_time(time: SystemTime) -> Option<String> {
    let secs = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60
    ))
}

/// The end of a file, at most [`MAX_BYTES`], starting at a line, as text. A whole JSON file,
/// such as Cinnabar's crash report, is pretty-printed.
pub fn read_tail(path: &Path) -> std::io::Result<(String, bool)> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let cut = len > MAX_BYTES;
    if cut {
        file.seek(SeekFrom::Start(len - MAX_BYTES))?;
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if cut {
        // Drop the partial first line.
        if let Some(i) = text.find('\n') {
            text.drain(..=i);
        }
    } else if path.extension().is_some_and(|e| e == "json") {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                text = pretty;
            }
        }
    }
    Ok((text, cut))
}

/// How a line should look: a problem, a warning, or plain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Plain,
}

pub fn level(line: &str) -> Level {
    let upper = line.to_ascii_uppercase();
    if upper.contains("ERROR")
        || upper.contains("FATAL")
        || upper.contains("PANICKED")
        || upper.contains("EXCEPTION")
    {
        Level::Error
    } else if upper.contains("WARN") {
        Level::Warn
    } else {
        Level::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_horizons_log_and_crash_reports() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join("logs")).unwrap();
        fs::create_dir_all(home.join("crash-reports")).unwrap();
        fs::write(home.join("logs/latest.log"), "hello").unwrap();
        fs::write(home.join("crash-reports/crash-1-client.txt"), "boom").unwrap();
        fs::write(home.join("crash-reports/notes.md"), "not a report").unwrap();
        let files = horizon_files(home);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "latest.log");
        assert_eq!(files[0].kind, Kind::Log);
        assert_eq!(files[1].kind, Kind::Crash);
        assert!(files[1].name.starts_with("Crash at "));
        assert!(horizon_files(&home.join("missing")).is_empty());
    }

    #[test]
    fn finds_cinnabars_logs_and_crash_folders() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        fs::create_dir_all(data.join("logs")).unwrap();
        fs::create_dir_all(data.join("crashes/20261101T1230Z-ab")).unwrap();
        fs::create_dir_all(data.join("crashes/empty")).unwrap();
        fs::write(data.join("logs/client.log"), "c").unwrap();
        fs::write(data.join("logs/core.log"), "g").unwrap();
        fs::write(data.join("crashes/20261101T1230Z-ab/report.json"), "{}").unwrap();
        let files = cinnabar_files(data);
        let names: Vec<_> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(&names[..2], ["client.log", "core.log"]);
        assert_eq!(files.len(), 3);
        assert_eq!(files[2].kind, Kind::Crash);
    }

    #[test]
    fn reads_only_the_end_of_a_long_file_from_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.log");
        let line = "x".repeat(99) + "\n";
        fs::write(&path, line.repeat((MAX_BYTES as usize / 100) + 50)).unwrap();
        let (text, cut) = read_tail(&path).unwrap();
        assert!(cut);
        assert!(text.len() as u64 <= MAX_BYTES);
        assert!(
            text.lines().all(|l| l.len() == 99),
            "starts at a whole line"
        );

        fs::write(&path, "short\nlog\n").unwrap();
        assert_eq!(
            read_tail(&path).unwrap(),
            ("short\nlog\n".to_string(), false)
        );

        let json = dir.path().join("report.json");
        fs::write(&json, r#"{"schema":1,"message":"boom"}"#).unwrap();
        let pretty = read_tail(&json).unwrap().0;
        assert!(
            pretty.contains("\n  \"message\": \"boom\""),
            "pretty-printed: {pretty}"
        );
    }

    #[test]
    fn formats_times_and_levels() {
        // Checked against `date -u -d @<seconds>`.
        let at = |secs| format_time(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs));
        assert_eq!(at(1_793_536_200).unwrap(), "2026-11-01 12:30 UTC");
        assert_eq!(at(951_782_400).unwrap(), "2000-02-29 00:00 UTC");
        assert_eq!(at(0).unwrap(), "1970-01-01 00:00 UTC");
        assert_eq!(level("[12:00] [main/ERROR]: boom"), Level::Error);
        assert_eq!(level("thread 'main' panicked at x"), Level::Error);
        assert_eq!(level("WARN something"), Level::Warn);
        assert_eq!(level("all good"), Level::Plain);
    }

    #[test]
    fn bedrock_roots_follow_the_installation() {
        let default = Installation::default_for(Edition::Bedrock);
        assert_eq!(root(&default), crate::cinnabar::data_dir());
        let mut list = crate::installations::Installations::default();
        let own = list.add(Edition::Bedrock);
        assert_eq!(root(&own), own.folder().map(|f| f.join("data")));
        let java = Installation::default_for(Edition::Java);
        assert_eq!(root(&java), Some(crate::horizon::home_dir()));
    }
}
