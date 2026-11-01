//! Ed25519 signatures over a release's `SHA256SUMS`, shared by Horizon's releases
//! ([`crate::horizon::sign`]) and RustLauncherMC's own ([`crate::update`]).
//!
//! A `SHA256SUMS.sig` file holds one base64 signature per line, so a key can be rotated by
//! signing with the old and new keys for a while. Each signature covers
//! `<project> release <tag>\n` followed by the bytes of `SHA256SUMS`, so a signed list can't be
//! passed off under another tag or for another project.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};

/// An Ed25519 public key.
pub type PublicKey = [u8; 32];

/// What a signature covers.
pub fn message(project: &str, tag: &str, sums: &[u8]) -> Vec<u8> {
    let mut message = format!("{project} release {tag}\n").into_bytes();
    message.extend_from_slice(sums);
    message
}

/// The keys in a keys file: one base64 key per line; blank lines and `#` comments are skipped.
pub fn parse_keys(text: &str) -> Result<Vec<PublicKey>> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let bytes = STANDARD
                .decode(line)
                .with_context(|| format!("{line}: not base64"))?;
            PublicKey::try_from(bytes.as_slice())
                .ok()
                .with_context(|| format!("{line}: not a 32-byte Ed25519 public key"))
        })
        .collect()
}

/// Whether any line of `signatures` is a valid signature of `message` by any of `keys`.
pub fn signed_by_any(message: &[u8], signatures: &str, keys: &[PublicKey]) -> bool {
    signatures
        .lines()
        .filter_map(|line| STANDARD.decode(line.trim()).ok())
        .any(|sig| {
            keys.iter().any(|key| {
                UnparsedPublicKey::new(&ED25519, key)
                    .verify(message, &sig)
                    .is_ok()
            })
        })
}

/// A new key pair: the private key (base64 PKCS#8, kept secret) and the public key (base64).
pub fn generate() -> Result<(String, String)> {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
        .ok()
        .context("could not generate a key")?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .ok()
        .context("could not read the new key")?;
    Ok((
        STANDARD.encode(pkcs8.as_ref()),
        STANDARD.encode(pair.public_key().as_ref()),
    ))
}

/// Signs `message` with a base64 PKCS#8 private key, as one base64 line.
pub fn sign(private_key: &str, message: &[u8]) -> Result<String> {
    let pkcs8 = STANDARD
        .decode(private_key.trim())
        .context("the private key is not base64")?;
    let pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
        .ok()
        .context("the private key is not an Ed25519 PKCS#8 key")?;
    Ok(STANDARD.encode(pair.sign(message).as_ref()))
}

/// The base64 public key of a base64 PKCS#8 private key.
pub fn public_key(private_key: &str) -> Result<String> {
    let pkcs8 = STANDARD.decode(private_key.trim())?;
    let Ok(pair) = Ed25519KeyPair::from_pkcs8(&pkcs8) else {
        bail!("the private key is not an Ed25519 PKCS#8 key");
    };
    Ok(STANDARD.encode(pair.public_key().as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_and_verifies() {
        let (private, public) = generate().unwrap();
        assert_eq!(public_key(&private).unwrap(), public);
        let keys = parse_keys(&format!("# comment\n\n{public}\n")).unwrap();
        let msg = message("rustlaunchermc", "v1.0.0", b"sums");
        let sig = sign(&private, &msg).unwrap();
        assert!(signed_by_any(&msg, &format!("garbage\n{sig}\n"), &keys));
        assert!(!signed_by_any(
            &message("rustlaunchermc", "v1.0.1", b"sums"),
            &sig,
            &keys
        ));
        assert!(!signed_by_any(&msg, &sig, &[]));
        assert!(parse_keys("not base64!").is_err());
    }
}
