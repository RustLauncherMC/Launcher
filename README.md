<p align="center">
  <img src="assets/banner.png" alt="RustLauncherMC: one launcher for Minecraft rewritten in Rust. Java with Horizon, Bedrock with Cinnabar, on Windows and Linux" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-early%20development-F0795A" alt="Status: early development">
  <img src="https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-2A2470" alt="Windows and Linux">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://rustlaunchermc.github.io/Launcher/"><b>Website</b></a> ·
  <a href="https://github.com/RustLauncherMC/Launcher/releases"><b>Download</b></a> ·
  <a href="docs/releasing.md"><b>Releasing</b></a>
</p>

# RustLauncherMC

**One launcher for both Rust editions of Minecraft.** RustLauncherMC installs, updates and launches
[Horizon](https://github.com/horizon-rust/horizon) (Java Edition, rewritten in Rust) and
[Cinnabar](https://github.com/cinnabar-mc/cinnabar) (a Bedrock Edition client written in Rust)
from one window. The interface follows the layout of the official Minecraft Launcher, so it feels
familiar from the first click.

> [!IMPORTANT]
> RustLauncherMC is a **fan project**. It is not an official Minecraft product and is not
> approved by or associated with Mojang or Microsoft. It is also not affiliated with the Horizon
> or Cinnabar projects. Minecraft is a trademark of Microsoft Corporation. The launcher ships no
> Minecraft code, assets or data.

> [!NOTE]
> Early development, with no release yet. When you build from source, Bedrock Edition (Cinnabar)
> can be installed and launched, and Java Edition (Horizon) can be installed, prepared and
> launched. Java sign-in works once Mojang approves the launcher's sign-in app.

## Why

Horizon and Cinnabar each ship their own launcher. If you play both editions, you keep two
launchers, two sign-ins and two update channels. RustLauncherMC puts them side by side, the same
way the official launcher puts Java and Bedrock side by side.

## What works today

- **The launcher window:** the official launcher's layout, with an edition sidebar, Play,
  Installations and Patch notes tabs, and a bottom bar with the channel and a big green Play
  button. Built with [GPUI](https://github.com/longbridge/gpui-fast), like Horizon's launcher.
- **Bedrock Edition through Cinnabar:**
  - Finds an existing Cinnabar install.
  - Installs the latest Beta or Nightly from GitHub, checked against the release's SHA-256 sums.
    On Linux it uses the same layout as Cinnabar's own `install.sh`; on Windows it runs the
    official MSI.
  - Shows the installed version and launches the game.
  - Shows the release notes.
- **Java Edition sign-in:** Microsoft sign-in with a code (microsoft.com/link), Xbox Live, and a
  check that the account owns Java Edition (Game Pass included). Several accounts can be signed
  in. Tokens are kept only in the system's password storage: Windows Credential Manager, or the
  Secret Service (GNOME Keyring, KWallet) on Linux.
- **Java Edition through Horizon:** Play does everything Horizon's own launcher does before a
  start:
  - Checks that the signed-in account owns Java Edition.
  - Installs or updates to the newest nightly, checked against Horizon's signed `SHA256SUMS`.
  - Prepares the game files with Horizon's preparer. The Minecraft files come from the official
    launcher's folder, or are downloaded from Mojang for the signed-in owner, each file checked
    against its SHA-1. Java 25 is downloaded from Mojang if the system has none.
  - Starts the game through Horizon's Launch API, handing over the account on standard input.
  - Shows the crash report if the game crashes.
- **Installations:** several per edition, like the official launcher's Installations tab, each
  with its own worlds and settings. Pick one beside Play. Java installations can open
  fullscreen.
- **Join a server:** type an address on the Play tab, and the game joins it as it starts.
- **Player heads:** each Java account shows its player's head.
- **Updates:** the launcher checks its own releases, verifies their signature, and updates
  itself.
- **Settings:** accounts, release channel, updates, folders, and license information.

## Download

From the [releases page](https://github.com/RustLauncherMC/Launcher/releases), once the first
release is out:

| System | File |
| --- | --- |
| Windows 10 and 11 (x64) | `RustLauncherMC-Setup-x64.exe` (installer, no administrator needed), or the portable `rustlauncher-windows-x64.zip` |
| Linux (x86-64) | `RustLauncherMC-x86_64.AppImage`, `rustlaunchermc_<version>_amd64.deb` for Debian and Ubuntu, or `rustlauncher-linux-x86_64.tar.gz` |

The installer, the AppImage and the zip and tar.gz builds update themselves. The `.deb` is
updated by installing the newer `.deb`. Windows may show a SmartScreen warning, because the
launcher is not code-signed yet.

## What you need

| | Java: Horizon | Bedrock: Cinnabar |
| --- | --- | --- |
| **Account** | A Microsoft account that owns Minecraft: Java Edition | A Microsoft account |
| **Game files** | Your own copy of Java Edition 26.3 (the launcher prepares Horizon's files from it, using Java 25) | Vanilla resource pack, downloaded on first launch after you accept the Minecraft EULA |
| **GPU** | Vulkan or OpenGL 3.3 (Linux), Vulkan or DirectX 12 (Windows) | See the Cinnabar README |

Each game's own README is the source of truth for its requirements. For how each game is
installed, signed in and launched, see [docs/upstream-research.md](docs/upstream-research.md).

## Platforms

| Platform | Support |
| --- | --- |
| Windows 10 and 11 (x64) | Supported (installer and portable zip) |
| Linux (x86-64), glibc 2.35+ | Supported (AppImage, .deb, tar.gz) |
| macOS | Not planned |

## Roadmap

- [x] Launcher window and official-style layout
- [x] Download, verify, install and launch Cinnabar releases
- [x] Cinnabar patch notes
- [x] Microsoft account sign-in (device code)
- [ ] Mojang's approval of the sign-in app
- [x] Download, verify, prepare and launch Horizon releases
- [x] Installations (profiles) per edition, and Join a server
- [x] Launcher self-update, with signed releases
- [x] Windows installer and Linux packages (AppImage, .deb)
- [ ] First release
- [ ] Translations

## Building

You need a recent stable Rust (tested with 1.97).

On Linux, install GPUI's libraries first. On Debian and Ubuntu:

```sh
sudo apt-get install libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libfontconfig-dev libvulkan-dev
```

Then:

```sh
git clone https://github.com/RustLauncherMC/Launcher.git
cd Launcher
cargo run --release
```

The workspace has two crates:

| Crate | What it is |
| --- | --- |
| [`crates/rlmc-core`](crates/rlmc-core) | The backend, with no UI: GitHub releases, verified downloads, settings, Microsoft sign-in and accounts, Horizon's install, preparation and launch, and Cinnabar's install, version and launch |
| [`crates/rlmc`](crates/rlmc) | The GPUI app (`rustlauncher` binary) |
| [`crates/rlmc-release`](crates/rlmc-release) | Signs releases; see [docs/releasing.md](docs/releasing.md) |

`cargo test --workspace` runs the tests. The Microsoft sign-in uses RustLauncherMC's own Azure app;
`RUSTLAUNCHERMC_MSA_CLIENT_ID` replaces its client ID at run time, for testing your own app. Set `RUSTLAUNCHERMC_HOME=/some/absolute/folder` to keep
the launcher's settings and downloads in a separate folder while testing.

## Website

The project's website is in [`website/`](website): plain HTML, CSS and JavaScript, published to
GitHub Pages by `.github/workflows/pages.yml`.

## Contributing

Ideas, issues and pull requests are welcome. Please open an issue first for larger changes so we
can agree on the approach.

## Branding

The logo is in [`assets/`](assets): `logo.svg` (source), `logo-256.png`, `logo-512.png`, and the
banner (`banner.svg`, `banner.png`). It shows a block with a Java side (Horizon purple), a Bedrock
side (Cinnabar red) and a play button, inside a Rust-orange cog. It uses no Mojang or Microsoft
artwork.

## License

[MIT](LICENSE). Bundled third-party parts are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Horizon (MIT) and Cinnabar (PolyForm
Noncommercial 1.0.0) are separate projects under their own licenses. The launcher downloads
them from their official releases and does not bundle them. Because of Cinnabar's license, RustLauncherMC is and stays noncommercial.
