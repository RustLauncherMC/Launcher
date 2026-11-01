//! SHA-256 checks against a release's `SHA256SUMS` file.

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The hash for `name` in a `sha256sum`-style file (`<hex>  <name>` or `<hex> *<name>`), in
/// lowercase. Like Cinnabar's own installer, it needs exactly one valid line for the name.
pub fn expected(sums: &str, name: &str) -> Result<String> {
    let mut found = None;
    let mut count = 0;
    for line in sums.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(file), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if file.strip_prefix('*').unwrap_or(file) == name {
            count += 1;
            found = Some(hash);
        }
    }
    match (count, found) {
        (1, Some(hash)) if is_sha256(hash) => Ok(hash.to_ascii_lowercase()),
        (0, _) => bail!("the release lists no checksum for {name}"),
        _ => bail!("the release has no single valid checksum for {name}"),
    }
}

fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The SHA-256 of a file, in lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    #[test]
    fn finds_text_and_binary_mode_lines() {
        let sums = format!("{A}  Cinnabar-x64.msi\n{B} *Cinnabar-x86_64.AppImage\n");
        assert_eq!(expected(&sums, "Cinnabar-x64.msi").unwrap(), A);
        assert_eq!(
            expected(&sums, "Cinnabar-x86_64.AppImage").unwrap(),
            B.to_ascii_lowercase()
        );
    }

    #[test]
    fn rejects_missing_duplicate_or_malformed() {
        assert!(expected(&format!("{A}  other\n"), "x").is_err());
        assert!(expected(&format!("{A}  x\n{A}  x\n"), "x").is_err());
        assert!(expected("abc  x\n", "x").is_err());
    }

    #[test]
    fn hashes_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
