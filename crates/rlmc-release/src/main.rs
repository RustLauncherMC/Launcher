//! Signs RustLauncherMC's releases. See docs/releasing.md.
//!
//! ```text
//! rlmc-release keygen                      prints a new private key, then its public key
//! rlmc-release sign <tag> <SHA256SUMS>     prints the signature line (key in RELEASE_SIGNING_KEY)
//! rlmc-release verify <tag> <SHA256SUMS> <SHA256SUMS.sig> [<keys file>]
//! ```

use anyhow::{bail, Context, Result};
use rlmc_core::signing;

const PROJECT: &str = "rustlaunchermc";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["keygen"] => {
            let (private, public) = signing::generate()?;
            println!("Private key (store it as the RELEASE_SIGNING_KEY secret; never commit it):");
            println!("{private}");
            println!("Public key (add it to release-keys.txt):");
            println!("{public}");
        }
        ["sign", tag, sums] => {
            let key = std::env::var("RELEASE_SIGNING_KEY")
                .context("set RELEASE_SIGNING_KEY to the private key")?;
            let sums = std::fs::read(sums).with_context(|| format!("read {sums}"))?;
            println!("{}", signing::sign(&key, &signing::message(PROJECT, tag, &sums))?);
        }
        ["verify", tag, sums, sig, rest @ ..] => {
            let keys_file = rest.first().copied().unwrap_or("release-keys.txt");
            let keys = signing::parse_keys(&std::fs::read_to_string(keys_file)?)?;
            let sums = std::fs::read(sums)?;
            let sig = std::fs::read_to_string(sig)?;
            if !signing::signed_by_any(&signing::message(PROJECT, tag, &sums), &sig, &keys) {
                bail!("the signature doesn't match any key in {keys_file}");
            }
            println!("Signature OK");
        }
        _ => bail!(
            "usage: rlmc-release keygen | sign <tag> <SHA256SUMS> | verify <tag> <SHA256SUMS> <SHA256SUMS.sig> [<keys file>]"
        ),
    }
    Ok(())
}
