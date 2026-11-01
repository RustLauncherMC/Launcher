<p align="center">
  <img src="assets/banner.png" alt="RustLauncherMC: one launcher for Minecraft rewritten in Rust. Java with Horizon, Bedrock with Cinnabar, on Windows and Linux" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-early%20development-F0795A" alt="Status: early development">
  <img src="https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-2A2470" alt="Windows and Linux">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
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
> Early development, with no release yet. Bedrock Edition (Cinnabar) can be installed and launched
> today when you build from source. Java Edition (Horizon) comes next.

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
- **Settings:** release channel, folders, and license information.

## Planned features

- **Familiar layout:** an edition sidebar (Java: Horizon, Bedrock: Cinnabar), a large Play button
  with a version picker, and Play, Installations and Patch notes tabs for each edition.
- **Sign-in:** Microsoft sign-in for Horizon, in the launcher. Cinnabar keeps its own sign-in and
  account switcher inside the game.
- **Install and update:** downloads each game from its project's GitHub releases and checks
  them against the published checksums. Horizon ships Nightly builds, and Cinnabar ships Beta and
  Nightly. Cinnabar keeps updating itself, and the launcher updates itself too.
- **Installations:** several profiles per edition, each with its own version, game folder and
  launch options.
- **Patch notes:** release notes from Horizon and Cinnabar, shown in the launcher.
- **Native and light:** written in Rust, a single small binary for each platform.

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
| Windows 10 and 11 (x64) | In development |
| Linux (x86-64) | In development |
| macOS | Not planned |

## Roadmap

- [x] Launcher window and official-style layout
- [x] Download, verify, install and launch Cinnabar releases
- [x] Cinnabar patch notes
- [ ] Microsoft account sign-in (device code), with our own Mojang-approved app
- [ ] Download, prepare and launch Horizon releases
- [ ] Installations (profiles) per edition
- [ ] Launcher self-update
- [ ] Windows installer and Linux packages (AppImage, .deb)

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
| [`crates/rlmc-core`](crates/rlmc-core) | The backend, with no UI: GitHub releases, verified downloads, settings, and Cinnabar's install, version and launch |
| [`crates/rlmc`](crates/rlmc) | The GPUI app (`rustlauncher` binary) |

`cargo test --workspace` runs the tests. Set `RUSTLAUNCHERMC_HOME=/some/absolute/folder` to keep
the launcher's settings and downloads in a separate folder while testing.

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
