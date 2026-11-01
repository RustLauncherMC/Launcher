//! Horizon's signed releases, checked the way Horizon's own launcher checks them
//! (`crates/horizon-release-sign` upstream, MIT).
//!
//! Every release carries `SHA256SUMS` (`<hex>  <file>` per line) and `SHA256SUMS.sig`: one base64
//! Ed25519 signature per line, each over `horizon release <tag>\n` followed by the bytes of
//! `SHA256SUMS`. Nightly builds are signed for the tag `nightly`, so a nightly's signature never
//! passes for a release, nor a release's for a nightly.

use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};

pub const SUMS: &str = "SHA256SUMS";
pub const SIGNATURE: &str = "SHA256SUMS.sig";
/// What nightly builds are signed for.
pub const NIGHTLY: &str = "nightly";

/// Horizon's release key, from `crates/horizon-release-sign/release-keys.txt` upstream.
pub const KEYS: &[&str] = &["MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw="];

fn keys() -> Vec<[u8; 32]> {
    KEYS.iter()
        .filter_map(|k| STANDARD.decode(k).ok())
        .filter_map(|k| <[u8; 32]>::try_from(k.as_slice()).ok())
        .collect()
}

/// What a signature covers: the tag, then the checksum list.
pub fn message(tag: &str, sums: &[u8]) -> Vec<u8> {
    let mut message = format!("horizon release {tag}\n").into_bytes();
    message.extend_from_slice(sums);
    message
}

/// Whether `tag` is a nightly's: `nightly`, or a past one kept as `nightly-<date>`.
pub fn is_nightly(tag: &str) -> bool {
    tag == NIGHTLY || tag.starts_with("nightly-")
}

/// Checks that Horizon's key signed `sums` for `tag` (`nightly` for any nightly).
pub fn verify(tag: &str, sums: &[u8], signature: &str) -> Result<()> {
    verify_with(tag, sums, signature, &keys())
}

fn verify_with(tag: &str, sums: &[u8], signature: &str, keys: &[[u8; 32]]) -> Result<()> {
    let signed_for = if is_nightly(tag) { NIGHTLY } else { tag };
    let message = message(signed_for, sums);
    let signed = signature
        .lines()
        .filter_map(|line| STANDARD.decode(line.trim()).ok())
        .any(|sig| {
            keys.iter().any(|key| {
                UnparsedPublicKey::new(&ED25519, key)
                    .verify(&message, &sig)
                    .is_ok()
            })
        });
    if !signed {
        bail!(
            "this Horizon build's signature doesn't match Horizon's release key, so it was not \
             installed. The download may have been tampered with"
        );
    }
    Ok(())
}

/// The version a nightly installs as, from its release title (`Nightly 2026-10-28 · 0c02067`):
/// `nightly-2026-10-28-0c02067`, as Horizon's launcher names it. `published` (GitHub's
/// `published_at`) dates a title without a date.
pub fn nightly_version(title: &str, published: &str) -> String {
    let mut words = title
        .strip_prefix("Nightly ")
        .unwrap_or_default()
        .split(" · ");
    let date = words
        .next()
        .filter(|d| is_date(d))
        .unwrap_or_else(|| published.get(..10).unwrap_or(published));
    let commit = words
        .next()
        .filter(|c| !c.is_empty() && c.chars().all(|c| c.is_ascii_hexdigit()));
    match commit {
        Some(commit) => format!("{NIGHTLY}-{date}-{commit}"),
        None => format!("{NIGHTLY}-{date}"),
    }
}

fn is_date(text: &str) -> bool {
    text.len() == 10
        && text.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn key_pair() -> Ed25519KeyPair {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()
    }

    #[test]
    fn accepts_a_nightly_signed_for_nightly_only() {
        let pair = key_pair();
        let key: [u8; 32] = pair.public_key().as_ref().try_into().unwrap();
        let sums = b"abc  horizon-linux-x86_64.tar.gz\n";
        let sig = STANDARD.encode(pair.sign(&message("nightly", sums)));
        assert!(verify_with("nightly", sums, &sig, &[key]).is_ok());
        assert!(verify_with("nightly-2026-10-28", sums, &sig, &[key]).is_ok());
        // The same signature is not valid for a release tag, or for other sums.
        assert!(verify_with("v1.0.0", sums, &sig, &[key]).is_err());
        assert!(verify_with("nightly", b"other", &sig, &[key]).is_err());
        // Nor with another key.
        let other: [u8; 32] = key_pair().public_key().as_ref().try_into().unwrap();
        assert!(verify_with("nightly", sums, &sig, &[other]).is_err());
    }

    #[test]
    fn the_built_in_key_parses() {
        assert_eq!(keys().len(), KEYS.len());
    }

    #[test]
    fn names_nightlies_like_horizon() {
        assert_eq!(
            nightly_version("Nightly 2026-10-28 · 0c02067", "2026-10-29T01:00:00Z"),
            "nightly-2026-10-28-0c02067"
        );
        assert_eq!(
            nightly_version("Nightly", "2026-10-29T01:00:00Z"),
            "nightly-2026-10-29"
        );
    }
}
