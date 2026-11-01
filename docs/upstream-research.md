# Upstream research: Horizon and Cinnabar

What RustLauncherMC needs to know to install, update and launch each game. This is based on a
read of both repositories on 2026-11-01:

- [horizon-rust/horizon](https://github.com/horizon-rust/horizon) (Java Edition rewritten in Rust, MIT)
- [cinnabar-mc/cinnabar](https://github.com/cinnabar-mc/cinnabar) (Bedrock client in Rust and Go, PolyForm Noncommercial)

Both projects move fast, so check paths and names against upstream before relying on them.

## At a glance

| | Horizon (Java) | Cinnabar (Bedrock) |
| --- | --- | --- |
| License | MIT | PolyForm Noncommercial 1.0.0 |
| Their own launcher | Separate app, `horizon-launcher` (GPUI) | None. The game's home menu is the launcher |
| Release channels | `nightly` only (old `vX.Y.Z` stay as "latest") | Beta (`vX.Y.Z`, "latest") and `nightly` |
| Windows asset | `horizon-windows-x64.zip` | `Cinnabar-x64.msi` / `Cinnabar-x64-setup.exe` |
| Linux asset | `horizon-linux-x86_64.tar.gz` | `Cinnabar-x86_64.AppImage` (+ `install.sh`) |
| Integrity | `SHA256SUMS` + Ed25519 `SHA256SUMS.sig`, public key in repo | `SHA256SUMS.txt`; signed update manifest, but its key is not public |
| Who signs in | **The launcher**, then it hands a session to the game | **The game** (its Go core) |
| Game files | Launcher prepares them from the user's own Java 26.3 copy (needs Java 25) | Game downloads Mojang's `bedrock-samples` after the user accepts the EULA |
| Who updates the game | The launcher | The game updates itself |
| How to launch | `horizon-client --resources … --session-stdin …` (Launch API) | `cinnabar` with no arguments |

## Horizon

### Releases
- **Game builds:** the rolling `nightly` pre-release. Older builds become `nightly-<date>` and are
  pruned after 7 days. Horizon's launcher names an installed build like
  `nightly-2026-10-28-0c02067` (`launcher/src/nightly.rs`).
- **Launcher builds:** tags `launcher@X.Y.Z`. We don't need these.
- **Finding builds:** there is no manifest. Read
  `GET https://api.github.com/repos/horizon-rust/horizon/releases?per_page=100` and match assets
  by name (`launcher/src/backend.rs`).
- **Downloads:**
  - URL pattern: `https://github.com/horizon-rust/horizon/releases/download/{tag}/{name}`
  - Assets: `horizon-windows-x64.zip`, `horizon-linux-x86_64.tar.gz`
  - Each archive has one top folder to strip.
- **Verification:**
  - Check `SHA256SUMS`, then `SHA256SUMS.sig`. The `.sig` file holds one base64 Ed25519 signature
    per line, over `horizon release <tag>\n` followed by the SHA256SUMS bytes.
  - Nightlies sign with `<tag>` = `nightly`.
  - Public key (`crates/horizon-release-sign/release-keys.txt`):
    `MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=`
  - The `horizon-release-sign` crate (MIT) does this.

### What a build contains
`horizon-client[.exe]`, `horizon-prepare`, `horizon-ctl`, `horizon-server`, `exporters/`,
`research-lock.json`, `tools/`, `LICENSE`.

### Preparing game files (first run, per Minecraft version)
1. **Pinned version:** Minecraft **26.3** only. `research-lock.json` pins the manifest, client
   and server sha1s, and `java_major: 25`.
2. **Find the user's Java Edition files.** Horizon reuses existing installs
   (`launcher/src/sources.rs`):
   - `.minecraft`
   - Prism, MultiMC, Modrinth App, ATLauncher, GDLauncher, and Flatpak paths
   - Every file is checked by sha1.
3. **Or download them from Mojang** (piston-meta, libraries, assets), but **only for an account
   that passed the ownership check**.
4. **Find or download Java 25** to run the exporters. Mojang's runtime needs glibc.
5. **Run the preparer:**
   ```
   horizon-prepare --prepare --server-jar J --minecraft-dir M --output DIR --complete --progress json [--java-home H]
   ```
   - It prints `{"message"}`, `{"fraction"}` and `{"error"}` lines; exit codes are 0, 1 and 2.
   - It needs about 1 GB of RAM and starts several JVMs.
   - The output must never be redistributed.

### Signing in (the launcher's job)
- **Flow:** Microsoft **device code** against the `consumers` tenant, scope
  `XboxLive.signin offline_access`. Then:
  1. Xbox Live user token
  2. XSTS (`rp://api.minecraftservices.com/`)
  3. `login_with_xbox`
  4. `/entitlements/mcstore` (must contain `game_minecraft` or `product_minecraft`)
  5. `/minecraft/profile`
- **We need our own Azure app client ID, approved by Mojang**
  ([review form](https://aka.ms/mce-reviewappid)). Horizon ships no ID, and reusing another app's
  is not allowed. **This is our main blocker for Java support.**
- **Token storage:** Horizon keeps tokens in the OS keychain (Windows Credential Manager, split
  into chunks because of a 2560-byte limit; Linux Secret Service). Only public data goes in
  `accounts.json`.
- **Ownership policy:** Horizon's launcher won't start the game without an account that owns
  Java Edition. Offline names are allowed only alongside one. We should follow the same policy.

### Launching: the Launch API
Horizon documents a **Launch API for third-party launchers** in `docs/api/launch.md` and
`docs/api/launch-reference.md`. It is version 0.1.0, marked experimental.

```
horizon-client --resources <prepared dir> --game-dir <instance> --home <instance> \
               --width W --height H --session-stdin
```
- **Discover options first:** run `horizon-client --launch-api`, which prints the option list as
  JSON. An unknown option exits with code 2.
- **Session:** send `{"name","uuid","access_token","user_type":"msa","xuid"}` on stdin. **Never
  put tokens in argv.** With no session, the game runs offline.
- **Extras:** `--quickPlaySingleplayer`, `--quickPlayMultiplayer`, `--server friend:CODE`.
- **Exit codes:** 0 ok, 1 error, 2 bad arguments, 101 panic.
  - The stdout line `Horizon: first frame` means the game started.
  - On a crash the last stderr line is `Horizon: crash report saved to <path>`.
- **Logs and crashes:** `<home>/logs/latest.log`; crash reports in
  `<home>/crash-reports/crash-*-client.txt`.
- **Windows:** spawn with `CREATE_NO_WINDOW`. If Vulkan crashes without a report, Horizon's
  launcher switches to DX12 by writing `graphicsApi:dx12` to `horizon-graphics.txt`.

### Code we can reuse (MIT)
- `horizon-release-sign`: signature checking.
- `horizon-prepare`: game-file preparation.
- `horizon-theme`.
- The session code in `launcher/src/account.rs`.
- Horizon's whole launcher (GPUI, about 138 files) is a good reference for UI, instances and
  self-update.
- **Notes:**
  - None of these crates are on crates.io, so use git dependencies or vendor them, and keep the
    MIT notice.
  - Horizon's self-update avoids the `self_replace` crate because Defender flagged it. On Windows
    it renames the running exe to `.old-<pid>-<n>` instead.

## Cinnabar

### Releases
- **Beta:** tags `vX.Y.Z`, published as GitHub "latest".
- **Nightly:** the rolling `nightly` prerelease, rebuilt daily.
- **Stable:** reserved for later.
- **Assets:**
  - Windows: `Cinnabar-x64.msi` (also used for updates) and `Cinnabar-x64-setup.exe`
  - Linux: `Cinnabar-x86_64.AppImage`, plus `install.sh`
  - Every release: `SHA256SUMS.txt`
- **Update manifests:** `releases/latest/download/update-beta.json` and
  `releases/download/nightly/update-nightly.json`.
  - They hold an Ed25519-signed envelope `{key_id, signature, manifest}`.
  - The trusted keys are compiled in at build time and are not in the repo, so we verify with
    `SHA256SUMS.txt` instead.
- **Signing:** builds may be unsigned, so expect SmartScreen warnings.

### Installed layout (`crates/launcher/src/install_layout.rs`)
- **Windows:**
  - The per-machine MSI installs into `Program Files\Cinnabar\`: `cinnabar.exe`,
    `bedrock-core.exe`, `resources\`, `dxcompiler.dll`. It may ask for administrator approval.
  - User data goes in `%LOCALAPPDATA%\Cinnabar\`.
- **Linux:**
  - `install.sh` unpacks the AppImage into `$XDG_DATA_HOME/cinnabar/app/<tag>-<digest>/`, with
    `app/current` and `app/previous` naming the active and previous builds.
  - It also writes the wrapper `~/.local/bin/cinnabar` and a `.desktop` entry.
  - Config goes in `$XDG_CONFIG_HOME/cinnabar`, data in `$XDG_DATA_HOME/cinnabar`.
- **Separate data per profile:** `CINNABAR_USER_ROOT=<abs path>` moves config, data and run into
  that folder. This gives us one folder per installation.

### What the game does itself (leave it alone)
- **Sign-in and account switching:** handled by the Go core (`bedrock-core`). Tokens live in
  `<data>/auth/`, in a lock-protected format the core owns. **We don't touch Cinnabar's tokens,**
  and we don't pass it an account.
- **EULA and vanilla assets:**
  - On first run the game asks for EULA consent, then downloads Mojang's `bedrock-samples`
    (pinned in `assets/vanilla-source.json`) and checks it by SHA-256.
  - `CINNABAR_ACCEPT_MOJANG_EULA=1` skips the prompt. **We should not set it:** the user should
    accept the EULA themselves.
- **Self-updates:** at launch and every 6 h, through `cinnabar-updater`. There is no environment
  variable to turn this off, only an in-game setting. Our launcher and the game would race on
  updates.
- **Discord:** rich presence and joins. The game registers itself as Discord's launch target,
  so Discord invites skip our launcher.
- **Packs and mods:** resource packs, shader packs and WebAssembly mods are managed in the game.

### Launching
- **Start:** run `cinnabar` with no arguments to open its menu.
- **Optional flags:** `--address HOST:PORT` (direct join), `--language ll_CC`, `--render-mode`.
- **Installed version:** `cinnabar --update-build-info` prints `{"version","number","channel?"}`
  and exits.
- **Don't call:** `--first-run-setup`, `--crash-monitor`, `--startup-screen` (internal).
- **Exit codes:** 0 ok, 130 Ctrl+C, 1 failure, 2 bad arguments.
- **Crashes and logs:**
  - The game runs its own crash window. Crash reports go to `<data>/crashes/<UTC>-<id>/report.json`.
  - Logs: `<data>/logs/client.log`, `core.log`.
- **Linux libraries:** Wayland, xkbcommon, ALSA, udev.
- **Graphics:** Vulkan or DX12, picked automatically; `WGPU_BACKEND` overrides it.

### License
PolyForm Noncommercial 1.0.0:
- **Allowed:** noncommercial use and redistribution, with the license attached.
- **Not allowed:** commercial use, or sublicensing.
- **What we do:** download official release assets from GitHub rather than bundling Cinnabar,
  keep RustLauncherMC noncommercial, and don't suggest endorsement.

## What this means for RustLauncherMC

1. **The two games need different integration depths.**
   - **Horizon:** our launcher does the full job (sign-in, ownership check, Java detection, file
     preparation, signed downloads, Launch API).
   - **Cinnabar:** our launcher installs, launches and steps aside.
2. **No single sign-in for both games.**
   - Cinnabar keeps its own credentials, so a "one account for both" feature is only possible on
     the Java side.
   - The Bedrock page can show "Sign in inside Cinnabar".
   - We could read the gamertag from `<data>/auth/account-manager/accounts.json` (read-only) to
     show who is signed in.
3. **Microsoft client ID.** Java support needs our own Azure app and Mojang's approval. Apply
   early, because it takes time. Until then, Java can launch offline only when the user has
   already signed in somewhere we can verify, so it is effectively blocked.
4. **Cinnabar updates.**
   - **Recommended:** let the game update itself. Our launcher installs the first copy, shows the
     installed version (`--update-build-info`) and the latest release, and offers to repair or
     reinstall.
   - Fighting the game's own updater would break things.
5. **Cinnabar on Windows needs admin rights.** The MSI installs per machine, so expect a UAC
   prompt. Alternatively, detect an existing install and reuse it.
6. **Profiles.**
   - Horizon instances: `--game-dir` / `--home` per instance.
   - Cinnabar installations: `CINNABAR_USER_ROOT` per installation.
   - Both fit the official launcher's "Installations" tab.
7. **Our own data folders:**
   - Windows: `%APPDATA%\RustLauncherMC`
   - Linux: `$XDG_DATA_HOME/rustlaunchermc` (`dirs::data_dir()`)
   - Don't share folders with `horizon-launcher` or Cinnabar.
8. **UI toolkit.** Horizon uses GPUI (a fork of Zed's GPUI), and copying its launcher would get
   us far, fast. The alternatives are `iced` and `egui`, which are easier to learn but further
   from an official-launcher look. This is still to decide.
9. **Licensing.**
   - Our code: MIT.
   - Cinnabar: noncommercial terms apply because we download and run it; we don't redistribute it.
   - Horizon code we reuse: keep the notices in `THIRD_PARTY_NOTICES.md`.

## Open questions
- Will we apply for an Azure client ID now, or start with Bedrock (which needs none) and add Java
  later?
- Which UI toolkit: GPUI (reuse Horizon), `iced`, or `egui`?
- Does Horizon's current nightly already have `--game-dir`, `--home` and `--session-stdin`?
  Check with `--launch-api`.
