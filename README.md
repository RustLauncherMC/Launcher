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
> The project is at the very start. There is no release yet, and the features below are the plan,
> not what works today.

## Why

Horizon and Cinnabar each ship their own launcher. If you play both editions, you keep two
launchers, two sign-ins and two update channels. RustLauncherMC puts them side by side, the same
way the official launcher puts Java and Bedrock side by side.

## Planned features

- **Familiar layout:** an edition sidebar (Java: Horizon, Bedrock: Cinnabar), a large Play button
  with a version picker, and Play, Installations and Patch notes tabs for each edition.
- **One sign-in:** a single Microsoft account (or several) shared by both games.
- **Install and update:** downloads each game from its project's GitHub releases, with Stable and
  Nightly channels, and updates the launcher itself.
- **Installations:** several profiles per edition, each with its own version, game folder and
  launch options.
- **Patch notes:** release notes from Horizon and Cinnabar, shown in the launcher.
- **Native and light:** written in Rust, a single small binary for each platform.

## What you need

| | Java: Horizon | Bedrock: Cinnabar |
| --- | --- | --- |
| **Account** | A Microsoft account that owns Minecraft: Java Edition | A Microsoft account |
| **Game files** | Your own copy of Java Edition (Horizon prepares its files from it) | Vanilla resource pack, downloaded on first launch after you accept the Minecraft EULA |
| **GPU** | Vulkan or OpenGL 3.3 (Linux), Vulkan or DirectX 12 (Windows) | See the Cinnabar README |

Each game's own README is the source of truth for its requirements.

## Platforms

| Platform | Support |
| --- | --- |
| Windows 10 and 11 (x64) | Planned |
| Linux (x86-64) | Planned |
| macOS | Not planned |

## Roadmap

- [ ] Launcher window and official-style layout
- [ ] Microsoft account sign-in (device code)
- [ ] Download and install Horizon releases
- [ ] Download and install Cinnabar releases
- [ ] Installations (profiles) per edition
- [ ] Patch notes feed
- [ ] Launcher self-update
- [ ] Windows installer and Linux packages (AppImage, .deb)

## Building

There is no code to build yet. Once there is, it will be a standard Cargo project:

```sh
git clone https://github.com/RustLauncherMC/Launcher.git
cd Launcher
cargo run --release
```

## Contributing

Ideas, issues and pull requests are welcome. Please open an issue first for larger changes so we
can agree on the approach.

## Branding

The logo is in [`assets/`](assets): `logo.svg` (source), `logo-256.png`, `logo-512.png`, and the
banner (`banner.svg`, `banner.png`). It shows a block with a Java side (Horizon purple), a Bedrock
side (Cinnabar red) and a play button, inside a Rust-orange cog. It uses no Mojang or Microsoft
artwork.

## License

[MIT](LICENSE). Horizon and Cinnabar are separate projects under their own licenses.
