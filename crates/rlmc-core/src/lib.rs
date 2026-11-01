//! RustLauncherMC's backend. Everything here is plain Rust with no UI, so it can be tested on
//! its own and reused by a command line later.
//!
//! - [`microsoft`] and [`accounts`]: Microsoft sign-in for Java Edition.
//! - [`horizon`]: Minecraft: Java Edition through Horizon (install, prepare, launch).
//! - [`cinnabar`]: Minecraft: Bedrock Edition through Cinnabar (find, install, launch).
//! - [`github`]: reading a project's releases.
//! - [`download`] and [`checksum`]: verified downloads.
//! - [`settings`] and [`paths`]: the launcher's own files.
//!
//! See `docs/upstream-research.md` for how each game is installed and started.

pub mod accounts;
pub mod checksum;
pub mod cinnabar;
pub mod download;
pub mod github;
pub mod horizon;
pub mod microsoft;
pub mod paths;
pub mod settings;

pub use ureq;

/// The User-Agent sent with every request, as GitHub's API requires one.
pub const USER_AGENT: &str = concat!("RustLauncherMC/", env!("CARGO_PKG_VERSION"));

/// One HTTP agent for the whole launcher. It follows redirects (GitHub release downloads go
/// through one) and gives up on a stalled connection instead of hanging forever.
pub fn http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
}
