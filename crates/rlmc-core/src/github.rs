//! A project's GitHub releases.

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// The release title, or the tag when it has none.
    pub fn title(&self) -> &str {
        self.name
            .as_deref()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&self.tag_name)
    }

    /// The publication day (`2026-10-31`), from GitHub's RFC 3339 time.
    pub fn date(&self) -> Option<&str> {
        self.published_at.as_deref().and_then(|t| t.get(..10))
    }
}

/// `which` is `latest` (GitHub's newest non-prerelease) or `tags/<tag>`.
pub fn release(agent: &ureq::Agent, repo: &str, which: &str) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{repo}/releases/{which}");
    agent
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("GET {url}"))
        .context("could not reach GitHub")?
        .into_json()
        .context("GitHub sent a release we could not read")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_github_release() {
        let json = r#"{
            "tag_name": "v0.9.0", "name": "", "body": "Notes", "published_at": "2026-10-31T12:00:00Z",
            "assets": [{"name": "SHA256SUMS.txt", "browser_download_url": "https://x/SHA256SUMS.txt", "size": 10}]
        }"#;
        let r: Release = serde_json::from_str(json).unwrap();
        assert_eq!(r.title(), "v0.9.0");
        assert_eq!(r.date(), Some("2026-10-31"));
        assert!(r.asset("SHA256SUMS.txt").is_some());
        assert!(r.asset("missing").is_none());
    }
}
