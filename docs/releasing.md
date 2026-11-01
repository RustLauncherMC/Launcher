# Releasing RustLauncherMC

Pushing a tag like `v0.2.0` runs `.github/workflows/release.yml`. It builds the launcher on
Linux and Windows and publishes a GitHub release with:

| File | What it is |
| --- | --- |
| `RustLauncherMC-Setup-x64.exe` | Windows installer. Installs for the current user into `%LOCALAPPDATA%\Programs\RustLauncherMC`, with no administrator approval, and adds a Start menu entry. |
| `rustlauncher-windows-x64.zip` | Windows, portable. Also what self-update downloads on Windows. |
| `RustLauncherMC-x86_64.AppImage` | Linux, any distribution with glibc 2.35+ (Ubuntu 22.04 and newer). Updates itself. |
| `rustlaunchermc_<version>_amd64.deb` | Debian and Ubuntu package. Updated through new `.deb` files, not by the launcher. |
| `rustlauncher-linux-x86_64.tar.gz` | Linux, plain binary. Also what self-update downloads for it. |
| `SHA256SUMS`, `SHA256SUMS.sig` | Checksums, and their Ed25519 signature. |

## One-time setup: the signing key

The launcher only updates itself to a release whose `SHA256SUMS` is signed by a key listed in
`release-keys.txt`, which is built into each launcher. Until a key is set up, releases are
published unsigned and launchers don't offer them as updates (the installers still work).

1. On your own computer, in the repository:

   ```sh
   cargo run -p rlmc-release -- keygen
   ```

   It prints a **private key** and a **public key**.
2. In GitHub: **Settings → Secrets and variables → Actions → New repository secret**. Name it
   `RELEASE_SIGNING_KEY` and paste the private key. Keep a copy somewhere safe, like a password
   manager. **Never commit it or paste it anywhere public.**
3. Add the public key as a new line in `release-keys.txt`, and commit that.

Launchers built after step 3 trust that key. To replace a key, add the new public key, sign
releases with the new secret for a while (older launchers still need the old key), then remove
the old key.

## Making a release

1. Set the new version in `Cargo.toml` (`[workspace.package] version`), run `cargo build` so
   `Cargo.lock` follows, and merge that to `main`.
2. Tag the merge and push the tag:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

   The workflow refuses a tag that doesn't match the version in `Cargo.toml`.
3. When the workflow is done, check the release page and edit its notes if needed.

To check a release by hand:

```sh
sha256sum -c SHA256SUMS --ignore-missing
cargo run -p rlmc-release -- verify v0.2.0 SHA256SUMS SHA256SUMS.sig
```

## How self-update works

At start, and from Settings → Updates, the launcher compares its version with the latest
release. When a newer one is signed with a trusted key, it offers to update:

- **AppImage:** the AppImage file is replaced.
- **Windows (installer or zip), Linux `.tar.gz`:** the executable is replaced. On Windows the
  running `rustlauncher.exe` is renamed to `rustlauncher.exe.old-<pid>` first, and deleted on the
  next start.
- **`.deb` in `/usr/bin`, and development builds:** no self-update. The launcher says why.

Set `RUSTLAUNCHERMC_NO_SELF_UPDATE=1` to turn self-update off.

Windows SmartScreen may warn about the installer and executable, because they are not
code-signed. That needs a code-signing certificate, which this fan project doesn't have yet.
