//! Minecraft's own files, which Horizon prepares its resources from.
//!
//! Horizon contains no Minecraft code or assets. The launcher either uses the player's existing
//! official install, or downloads the pinned version from Mojang, **only for a signed-in account
//! that owns Java Edition**, as Horizon's launcher does. Files go in the official launcher's
//! layout (`versions/`, `libraries/`, `assets/`) and each is checked against Mojang's SHA-1.
//! The server jar (which no launcher keeps) and Mojang's Java runtime are downloaded the same way.

use super::Pins;
use crate::download::Progress;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const VERSIONS: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const OBJECTS: &str = "https://resources.download.minecraft.net";
const JAVA_RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
/// Files downloaded at once.
const JOBS: usize = 8;

/// Mojang's name for this system in library rules.
fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}

/// Mojang's name for this system in its Java runtime index.
fn java_platform() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        _ => return None,
    })
}

/// The official launcher's folder, where an existing install would be.
pub fn official_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir().map(|d| d.join(".minecraft"))
    } else {
        dirs::home_dir().map(|d| d.join(".minecraft"))
    }
}

/// Whether `dir` holds the pinned version's game: its version file and an untouched client jar.
pub fn has_game(dir: &Path, pins: &Pins) -> bool {
    let versions = dir.join("versions").join(&pins.version);
    versions.join(format!("{}.json", pins.version)).is_file()
        && sha1_file(&versions.join(format!("{}.jar", pins.version))).as_deref()
            == Some(pins.client_sha1.as_str())
}

/// The version file in `dir`, once downloaded or found.
pub fn version_json(dir: &Path, pins: &Pins) -> Result<Value> {
    let path = dir
        .join("versions")
        .join(&pins.version)
        .join(format!("{}.json", pins.version));
    serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {}", path.display()))?)
        .context("Minecraft's version file is unreadable")
}

struct File {
    url: String,
    sha1: String,
    path: PathBuf,
}

/// Downloads the pinned Minecraft version into `dir` (the official layout). Files already there
/// and whole are kept, so a stopped download goes on where it stopped.
pub fn download_game(
    agent: &ureq::Agent,
    pins: &Pins,
    dir: &Path,
    progress: &Progress,
) -> Result<()> {
    let v = &pins.version;
    progress.set_step(format!("Downloading Minecraft {v} from Mojang"));
    let list: Value = agent
        .get(VERSIONS)
        .call()
        .context("could not reach Mojang")?
        .into_json()?;
    let entry = list["versions"]
        .as_array()
        .and_then(|vs| vs.iter().find(|e| e["id"] == v.as_str()))
        .with_context(|| format!("Mojang has no Minecraft {v}"))?;
    let manifest_sha1 = text(entry, "sha1")?;
    if !pins.manifest_sha1s.contains(&manifest_sha1) {
        bail!("Mojang's Minecraft {v} is not the one this Horizon build was made for");
    }
    let versions = dir.join("versions").join(v);
    fetch(
        agent,
        &File {
            url: text(entry, "url")?,
            sha1: manifest_sha1,
            path: versions.join(format!("{v}.json")),
        },
    )?;
    let manifest = version_json(dir, pins)?;

    let client = &manifest["downloads"]["client"];
    let mut files = vec![File {
        url: text(client, "url")?,
        sha1: pins.client_sha1.clone(),
        path: versions.join(format!("{v}.jar")),
    }];
    let os = os_name();
    for library in manifest["libraries"]
        .as_array()
        .context("Minecraft's version file lists no libraries")?
        .iter()
        .filter(|l| allowed(l, os))
    {
        let native = native_classifier(library, os)
            .map(|key| &library["downloads"]["classifiers"][key.as_str()]);
        for artifact in [Some(&library["downloads"]["artifact"]), native]
            .into_iter()
            .flatten()
            .filter(|a| !a.is_null())
        {
            files.push(File {
                url: text(artifact, "url")?,
                sha1: text(artifact, "sha1")?,
                path: dir
                    .join("libraries")
                    .join(relative(&text(artifact, "path")?)?),
            });
        }
    }
    let logging = &manifest["logging"]["client"]["file"];
    if !logging.is_null() {
        files.push(File {
            url: text(logging, "url")?,
            sha1: text(logging, "sha1")?,
            path: dir
                .join("assets/log_configs")
                .join(relative(&text(logging, "id")?)?),
        });
    }
    fetch_all(agent, &files, progress)?;

    progress.set_step("Downloading Minecraft's sounds and languages");
    let index = &manifest["assetIndex"];
    let index_file = File {
        url: text(index, "url")?,
        sha1: text(index, "sha1")?,
        path: dir
            .join("assets/indexes")
            .join(relative(&format!("{}.json", text(index, "id")?))?),
    };
    fetch(agent, &index_file)?;
    let index: Value = serde_json::from_slice(&fs::read(&index_file.path)?)
        .context("Minecraft's asset index is unreadable")?;
    let objects = index["objects"]
        .as_object()
        .context("Minecraft's asset index lists no files")?
        .values()
        .map(|object| {
            let hash = text(object, "hash")?;
            if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("Minecraft's asset index has a bad hash");
            }
            Ok(File {
                url: format!("{OBJECTS}/{}/{hash}", &hash[..2]),
                path: dir.join("assets/objects").join(&hash[..2]).join(&hash),
                sha1: hash,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    fetch_all(agent, &objects, progress)
}

/// Downloads the pinned server jar to `dest`. Horizon's preparer needs it once.
pub fn download_server(
    agent: &ureq::Agent,
    manifest: &Value,
    pins: &Pins,
    dest: &Path,
    progress: &Progress,
) -> Result<()> {
    progress.set_step(format!("Downloading the Minecraft {} server", pins.version));
    let server = &manifest["downloads"]["server"];
    fetch(
        agent,
        &File {
            url: text(server, "url")?,
            sha1: pins.server_sha1.clone(),
            path: dest.to_path_buf(),
        },
    )
}

/// A Java home of the pinned major version for Horizon's preparer: `None` when the system's
/// Java already is that version (the preparer finds it itself), else Mojang's runtime, downloaded
/// into `runtimes` as the official launcher lays it out.
pub fn java_home(
    agent: &ureq::Agent,
    manifest: &Value,
    major: u32,
    runtimes: &Path,
    progress: &Progress,
) -> Result<Option<PathBuf>> {
    if system_java_major() == Some(major) {
        return Ok(None);
    }
    let component = text(&manifest["javaVersion"], "component")
        .context("Minecraft's version file names no Java runtime")?;
    let platform = java_platform().context("Mojang has no Java runtime for this computer")?;
    progress.set_step(format!("Downloading Java {major} from Mojang"));
    let index: Value = agent
        .get(JAVA_RUNTIMES)
        .call()
        .context("could not reach Mojang")?
        .into_json()?;
    let entry = &index[platform][component.as_str()][0];
    if entry.is_null() {
        bail!("Mojang has no {component} for {platform}");
    }
    let manifest_ref = &entry["manifest"];
    let list_path = runtimes.join(format!("{component}-{platform}.json"));
    fetch(
        agent,
        &File {
            url: text(manifest_ref, "url")?,
            sha1: text(manifest_ref, "sha1")?,
            path: list_path.clone(),
        },
    )?;
    let list: Value = serde_json::from_slice(&fs::read(&list_path)?)?;
    let home = runtimes.join(&component).join(platform).join(&component);
    let mut files = Vec::new();
    let mut executables = Vec::new();
    let mut links = Vec::new();
    for (name, file) in list["files"]
        .as_object()
        .context("Mojang's Java runtime lists no files")?
    {
        let path = home.join(relative(name)?);
        match file["type"].as_str() {
            Some("file") => {
                let raw = &file["downloads"]["raw"];
                if file["executable"] == true {
                    executables.push(path.clone());
                }
                files.push(File {
                    url: text(raw, "url")?,
                    sha1: text(raw, "sha1")?,
                    path,
                });
            }
            Some("link") => links.push((path, text(file, "target")?)),
            _ => {}
        }
    }
    fetch_all(agent, &files, progress)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in &executables {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        for (path, target) in &links {
            if !link_stays_inside(&home, path, target) {
                bail!("unsafe link {target:?} in Mojang's Java runtime");
            }
            let _ = fs::remove_file(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(target, path)?;
        }
    }
    #[cfg(not(unix))]
    let _ = (executables, links);
    Ok(Some(home))
}

/// The major version of the Java the preparer would find first (`JAVA_HOME`, then `PATH`).
fn system_java_major() -> Option<u32> {
    let java = std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin").join("java"))
        .filter(|p| p.is_file() || p.with_extension("exe").is_file())
        .unwrap_or_else(|| PathBuf::from("java"));
    let output = Command::new(java)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .ok()?;
    java_major(&String::from_utf8_lossy(&output.stderr))
}

/// `openjdk version "25.0.1" 2025-10-21` → 25; `"1.8.0_402"` → 8. Other lines, such as
/// "Picked up JAVA_TOOL_OPTIONS: …", are skipped.
fn java_major(version_output: &str) -> Option<u32> {
    let line = version_output.lines().find(|l| l.contains(" version \""))?;
    let version = line.split('"').nth(1)?;
    let mut parts = version.split(['.', '_', '-', '+']);
    match parts.next()?.parse().ok()? {
        1 => parts.next()?.parse().ok(),
        major => Some(major),
    }
}

// --- Rules and paths ------------------------------------------------------------------------

/// Whether a library's rules let this system (`windows`, `linux`, `osx`) use it: the last rule
/// that matches decides; with no rules, every system does.
fn allowed(library: &Value, os: &str) -> bool {
    let Some(rules) = library["rules"].as_array() else {
        return true;
    };
    rules
        .iter()
        .rfind(|rule| rule["os"]["name"].as_str().is_none_or(|name| name == os))
        .is_some_and(|rule| rule["action"] == "allow")
}

/// The classifier holding a library's natives for `os`, for versions before 1.19.
fn native_classifier(library: &Value, os: &str) -> Option<String> {
    library["natives"][os]
        .as_str()
        .map(|key| key.replace("${arch}", "64"))
}

/// A relative path from Mojang's files, refused if it could leave its folder.
fn relative(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        Ok(path)
    } else {
        bail!("unsafe path in Mojang's files: {}", path.display())
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
fn link_stays_inside(home: &Path, link: &Path, target: &str) -> bool {
    let Ok(within) = link.strip_prefix(home) else {
        return false;
    };
    // How deep the link sits below `home`, against how far up its target climbs.
    let mut depth = within.components().count() as i64 - 1;
    for component in Path::new(target).components() {
        match component {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            _ => return false,
        }
        if depth < 0 {
            return false;
        }
    }
    true
}

fn text(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .map(str::to_string)
        .with_context(|| format!("Mojang's files are missing {key:?}"))
}

// --- Downloading ----------------------------------------------------------------------------

fn sha1_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(crate::checksum::hex(&hasher.finalize()))
}

/// Downloads one file unless it is already there and whole, checking its SHA-1 before it
/// takes its final name.
fn fetch(agent: &ureq::Agent, file: &File) -> Result<()> {
    if sha1_file(&file.path).as_deref() == Some(file.sha1.as_str()) {
        return Ok(());
    }
    if !file.url.starts_with("https://") {
        bail!("refusing a download that is not HTTPS: {}", file.url);
    }
    let parent = file.path.parent().context("bad download path")?;
    fs::create_dir_all(parent)?;
    let mut last = None;
    for _ in 0..3 {
        match fetch_once(agent, file, parent) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap())
}

fn fetch_once(agent: &ureq::Agent, file: &File, parent: &Path) -> Result<()> {
    let mut reader = agent
        .get(&file.url)
        .call()
        .with_context(|| format!("GET {}", file.url))
        .context("a download from Mojang failed")?
        .into_reader();
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        std::io::Write::write_all(&mut temp, &buf[..n])?;
    }
    if crate::checksum::hex(&hasher.finalize()) != file.sha1 {
        bail!("a file from Mojang failed its SHA-1 check: {}", file.url);
    }
    temp.persist(&file.path)?;
    Ok(())
}

/// Downloads `files`, several at a time, counting them in `progress`.
fn fetch_all(agent: &ureq::Agent, files: &[File], progress: &Progress) -> Result<()> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let error = Mutex::new(None);
    progress.set_counts(0, files.len() as u64);
    std::thread::scope(|scope| {
        for _ in 0..JOBS.min(files.len()) {
            scope.spawn(|| loop {
                if error.lock().unwrap().is_some() {
                    return;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(file) = files.get(i) else { return };
                if let Err(e) = fetch(agent, file) {
                    error.lock().unwrap().get_or_insert(e);
                    return;
                }
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                progress.set_counts(n as u64, files.len() as u64);
            });
        }
    });
    match error.into_inner().unwrap() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn library_rules_follow_the_last_match() {
        let any = json!({});
        let linux_only = json!({"rules": [{"action": "allow", "os": {"name": "linux"}}]});
        let not_osx =
            json!({"rules": [{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}]});
        assert!(allowed(&any, "windows"));
        assert!(allowed(&linux_only, "linux"));
        assert!(!allowed(&linux_only, "windows"));
        assert!(allowed(&not_osx, "windows"));
        assert!(!allowed(&not_osx, "osx"));
    }

    #[test]
    fn old_natives_use_this_systems_classifier() {
        let lib = json!({"natives": {"windows": "natives-windows-${arch}"}});
        assert_eq!(
            native_classifier(&lib, "windows").as_deref(),
            Some("natives-windows-64")
        );
        assert_eq!(native_classifier(&lib, "linux"), None);
    }

    #[test]
    fn paths_and_links_stay_inside() {
        assert!(relative("a/b.jar").is_ok());
        assert!(relative("../x").is_err());
        assert!(relative("/etc/x").is_err());
        let home = Path::new("/r/java");
        assert!(link_stays_inside(
            home,
            Path::new("/r/java/bin/java"),
            "../lib/x"
        ));
        assert!(!link_stays_inside(
            home,
            Path::new("/r/java/bin/java"),
            "../../x"
        ));
        assert!(!link_stays_inside(
            home,
            Path::new("/r/java/bin/java"),
            "/usr/bin/java"
        ));
    }

    #[test]
    fn reads_java_versions() {
        assert_eq!(
            java_major(r#"openjdk version "25.0.1" 2025-10-21"#),
            Some(25)
        );
        assert_eq!(java_major(r#"java version "1.8.0_402""#), Some(8));
        assert_eq!(java_major(r#"openjdk version "21" 2023-09-19"#), Some(21));
        assert_eq!(java_major("nothing"), None);
        assert_eq!(
            java_major("Picked up JAVA_TOOL_OPTIONS: -Dx=\"y\"\nopenjdk version \"25\" 2025-09-16"),
            Some(25)
        );
    }
}
