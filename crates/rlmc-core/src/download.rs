//! Downloads with progress that the UI can read from another thread.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// What a long job is doing, shared with the UI. Every field can be read at any time.
#[derive(Debug, Default)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
    step: Mutex<String>,
}

impl Progress {
    pub fn set_step(&self, step: impl Into<String>) {
        *self.step.lock().unwrap() = step.into();
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
    }

    /// For work counted in files or steps rather than bytes.
    pub fn set_counts(&self, done: u64, total: u64) {
        self.total.store(total, Ordering::Relaxed);
        self.done.store(done, Ordering::Relaxed);
    }

    /// For work that reports how far it is, from 0 to 1.
    pub fn set_fraction(&self, fraction: f64) {
        self.set_counts((fraction.clamp(0.0, 1.0) * 1000.0) as u64, 1000);
    }

    pub fn step(&self) -> String {
        self.step.lock().unwrap().clone()
    }

    /// From 0 to 1, or `None` while the size is unknown.
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0)
            .then(|| (self.done.load(Ordering::Relaxed) as f64 / total as f64).min(1.0) as f32)
    }
}

/// A small text file, such as `SHA256SUMS.txt`.
pub fn text(agent: &ureq::Agent, url: &str) -> Result<String> {
    let response = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))
        .context("could not reach GitHub")?;
    let mut body = String::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_string(&mut body)?;
    Ok(body)
}

/// Downloads `url` to `dest` and checks it against `sha256` (lowercase hex). On a mismatch the
/// file is deleted, so nothing unverified is left behind.
pub fn verified(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    sha256: &str,
    progress: &Progress,
) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("refusing a download that is not HTTPS: {url}");
    }
    let response = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))
        .context("the download could not start")?;
    if let Some(len) = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
    {
        progress.total.store(len, Ordering::Relaxed);
    }
    let mut reader = response.into_reader();
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 256 * 1024];
    loop {
        let n = reader.read(&mut buf).context("download interrupted")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        progress.done.fetch_add(n as u64, Ordering::Relaxed);
    }
    file.sync_all()?;
    drop(file);
    let actual = crate::checksum::hex(&hasher.finalize());
    if actual != sha256 {
        let _ = std::fs::remove_file(dest);
        bail!("the download failed its SHA-256 check, so nothing was installed. Please try again");
    }
    Ok(())
}
