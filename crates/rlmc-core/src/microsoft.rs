//! Microsoft sign-in for Minecraft: Java Edition.
//!
//! 1. Microsoft device code (`consumers` tenant, `XboxLive.signin offline_access`): the player
//!    opens microsoft.com/link and types a short code; the launcher polls for the result.
//! 2. Xbox Live user token, then an XSTS token for `rp://api.minecraftservices.com/`.
//! 3. `login_with_xbox`: the Minecraft access token (valid for about 24 hours).
//! 4. Entitlements: the account must own Java Edition (Game Pass included).
//! 5. Profile: the player's UUID and name.
//!
//! The client ID is RustLauncherMC's own Azure app. Using another application's ID is not
//! allowed. `RUSTLAUNCHERMC_MSA_CLIENT_ID` replaces it at run time, for testing another app.

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// RustLauncherMC's Azure app ("Personal Microsoft accounts only", public client flows on).
pub const CLIENT_ID: &str = "01fcbd6d-d954-499a-8c3a-0c837eef86ac";
pub const CLIENT_ID_ENV: &str = "RUSTLAUNCHERMC_MSA_CLIENT_ID";
const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

pub fn client_id() -> String {
    std::env::var(CLIENT_ID_ENV)
        .ok()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// --- HTTP helpers -------------------------------------------------------------------------

/// A response body, whatever the status: ureq reports 4xx and 5xx as errors.
fn body(result: Result<ureq::Response, ureq::Error>, what: &str) -> Result<(u16, String)> {
    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(e) => return Err(anyhow!(e).context(format!("could not reach {what}"))),
    };
    let status = response.status();
    let text = response
        .into_string()
        .with_context(|| format!("{what} sent an unreadable answer"))?;
    Ok((status, text))
}

fn parse<T: DeserializeOwned>(text: &str, what: &str) -> Result<T> {
    serde_json::from_str(text).with_context(|| format!("{what} sent an answer we could not read"))
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: String,
}

// --- 1. Microsoft ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    /// The code the player types, such as `ABCD1234`.
    pub user_code: String,
    /// Where they type it (https://www.microsoft.com/link).
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Clone, Debug, Deserialize)]
pub struct MsTokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// Starts a sign-in: the code to show the player.
pub fn device_code(agent: &ureq::Agent) -> Result<DeviceCode> {
    let client_id = client_id();
    let (status, text) = body(
        agent
            .post(&format!("{AUTHORITY}/devicecode"))
            .send_form(&[("client_id", &client_id), ("scope", SCOPE)]),
        "Microsoft",
    )?;
    if status != 200 {
        let error: OAuthError = parse(&text, "Microsoft")?;
        return Err(app_error(&error));
    }
    parse(&text, "Microsoft")
}

/// A short message for players, with Microsoft's full description kept underneath for the log.
fn app_error(error: &OAuthError) -> anyhow::Error {
    let code = aadsts_code(&error.error_description);
    let short = match error.error.as_str() {
        "unauthorized_client" | "invalid_client" => format!(
            "Microsoft refused this launcher's sign-in app{code}. It must allow personal \
             Microsoft accounts and public client flows"
        ),
        _ => format!("Microsoft sign-in failed{code}"),
    };
    anyhow!("{}", first_line(&error.error_description)).context(short)
}

/// ` (AADSTS700016)` from a description that starts with it, else nothing.
fn aadsts_code(description: &str) -> String {
    description
        .split(':')
        .next()
        .filter(|c| c.starts_with("AADSTS") && c[6..].bytes().all(|b| b.is_ascii_digit()))
        .map(|c| format!(" ({c})"))
        .unwrap_or_default()
}

fn first_line(text: &str) -> &str {
    let line = text.lines().next().unwrap_or(text);
    // Microsoft appends "Trace ID: …" to every description.
    line.split(" Trace ID:").next().unwrap_or(line).trim()
}

/// Waits until the player has entered the code, polling as Microsoft asks. `cancel` stops it.
pub fn wait_for_sign_in(
    agent: &ureq::Agent,
    code: &DeviceCode,
    cancel: &AtomicBool,
) -> Result<MsTokens> {
    let client_id = client_id();
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.max(1));
    loop {
        // Sleep in small steps so a cancel is noticed quickly.
        let wake = Instant::now() + interval;
        while Instant::now() < wake {
            if cancel.load(Ordering::Relaxed) {
                bail!("Sign-in cancelled");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if Instant::now() > deadline {
            bail!("The code expired. Please sign in again");
        }
        let (status, text) = body(
            agent.post(&format!("{AUTHORITY}/token")).send_form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", &client_id),
                ("device_code", &code.device_code),
            ]),
            "Microsoft",
        )?;
        if status == 200 {
            return parse(&text, "Microsoft");
        }
        let error: OAuthError = parse(&text, "Microsoft")?;
        match error.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += Duration::from_secs(5),
            "authorization_declined" => bail!("Sign-in was declined"),
            "expired_token" | "bad_verification_code" => {
                bail!("The code expired. Please sign in again")
            }
            _ => return Err(app_error(&error)),
        }
    }
}

/// The error for a refresh token Microsoft no longer accepts.
#[derive(Debug)]
pub struct SignInAgain;

impl std::fmt::Display for SignInAgain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("This account's sign-in has expired. Please sign in again")
    }
}

impl std::error::Error for SignInAgain {}

/// New tokens from a refresh token.
pub fn refresh(agent: &ureq::Agent, refresh_token: &str) -> Result<MsTokens> {
    let client_id = client_id();
    let (status, text) = body(
        agent.post(&format!("{AUTHORITY}/token")).send_form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ]),
        "Microsoft",
    )?;
    if status == 200 {
        return parse(&text, "Microsoft");
    }
    let error: OAuthError = parse(&text, "Microsoft")?;
    if error.error == "invalid_grant" {
        return Err(SignInAgain.into());
    }
    Err(app_error(&error))
}

/// The error for an account that doesn't own Java Edition.
#[derive(Debug)]
pub struct NotOwned;

impl std::fmt::Display for NotOwned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("This account doesn't own Minecraft: Java Edition")
    }
}

impl std::error::Error for NotOwned {}

// --- 2. Xbox Live ---------------------------------------------------------------------------

struct Xsts {
    token: String,
    uhs: String,
    xuid: Option<String>,
}

fn xbox_live(agent: &ureq::Agent, ms_access_token: &str) -> Result<(String, String)> {
    let (status, text) = body(
        agent
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .set("Accept", "application/json")
            .send_json(json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={ms_access_token}"),
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT",
            })),
        "Xbox Live",
    )?;
    if status != 200 {
        bail!("Xbox Live refused the sign-in (HTTP {status})");
    }
    let value: Value = parse(&text, "Xbox Live")?;
    let token = value["Token"].as_str().context("Xbox Live sent no token")?;
    let uhs = value["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .context("Xbox Live sent no user hash")?;
    Ok((token.to_string(), uhs.to_string()))
}

fn xsts(agent: &ureq::Agent, xbl_token: &str) -> Result<Xsts> {
    let (status, text) = body(
        agent
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .set("Accept", "application/json")
            .send_json(json!({
                "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT",
            })),
        "Xbox Live",
    )?;
    let value: Value = parse(&text, "Xbox Live").unwrap_or(Value::Null);
    if status != 200 {
        bail!(xsts_error(value["XErr"].as_u64(), status));
    }
    let claims = &value["DisplayClaims"]["xui"][0];
    Ok(Xsts {
        token: value["Token"]
            .as_str()
            .context("Xbox Live sent no token")?
            .to_string(),
        uhs: claims["uhs"]
            .as_str()
            .context("Xbox Live sent no user hash")?
            .to_string(),
        xuid: claims["xid"].as_str().map(str::to_string),
    })
}

fn xsts_error(xerr: Option<u64>, status: u16) -> String {
    match xerr {
        Some(2148916233) => "This Microsoft account has no Xbox profile yet. Sign in once at \
                             xbox.com to create one, then try again"
            .into(),
        Some(2148916235) => "Xbox Live is not available in this account's country".into(),
        Some(2148916236) | Some(2148916237) => {
            "This account needs adult verification on xbox.com first".into()
        }
        Some(2148916238) => "This is a child account. An adult must add it to a Microsoft \
                             family before it can sign in"
            .into(),
        Some(code) => format!("Xbox Live refused the sign-in (error {code})"),
        None => format!("Xbox Live refused the sign-in (HTTP {status})"),
    }
}

// --- 3-5. Minecraft --------------------------------------------------------------------------

/// A signed-in Minecraft account.
#[derive(Clone, Debug)]
pub struct Minecraft {
    pub access_token: String,
    /// Unix time when `access_token` stops working.
    pub expires_at: u64,
    pub uuid: String,
    pub name: String,
    pub xuid: Option<String>,
}

const SERVICES: &str = "https://api.minecraftservices.com";

/// From a Microsoft access token to a Minecraft account that owns Java Edition.
pub fn minecraft(agent: &ureq::Agent, ms_access_token: &str) -> Result<Minecraft> {
    let (xbl, _) = xbox_live(agent, ms_access_token)?;
    let xsts = xsts(agent, &xbl)?;

    let (status, text) = body(
        agent
            .post(&format!("{SERVICES}/authentication/login_with_xbox"))
            .send_json(json!({
                "identityToken": format!("XBL3.0 x={};{}", xsts.uhs, xsts.token),
            })),
        "Minecraft",
    )?;
    if status == 403 && text.contains("Invalid app registration") {
        bail!(
            "Mojang has not approved this launcher's sign-in app yet, so Minecraft refused the \
             sign-in. Please try again once it is approved"
        );
    }
    if status != 200 {
        bail!("Minecraft refused the sign-in (HTTP {status})");
    }
    #[derive(Deserialize)]
    struct Login {
        access_token: String,
        expires_in: u64,
    }
    let login: Login = parse(&text, "Minecraft")?;

    if !owns_java(agent, &login.access_token)? {
        return Err(NotOwned.into());
    }
    let (uuid, name) = profile(agent, &login.access_token)?;
    Ok(Minecraft {
        expires_at: now() + login.expires_in,
        access_token: login.access_token,
        uuid,
        name,
        xuid: xsts.xuid,
    })
}

/// Whether the account owns Java Edition, Game Pass included.
pub fn owns_java(agent: &ureq::Agent, mc_token: &str) -> Result<bool> {
    let (status, text) = body(
        agent
            .get(&format!("{SERVICES}/entitlements/mcstore"))
            .set("Authorization", &format!("Bearer {mc_token}"))
            .call(),
        "Minecraft",
    )?;
    if status != 200 {
        bail!("Minecraft could not check this account's games (HTTP {status})");
    }
    Ok(owns_from(&parse(&text, "Minecraft")?))
}

fn owns_from(entitlements: &Value) -> bool {
    entitlements["items"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            matches!(
                item["name"].as_str(),
                Some("game_minecraft") | Some("product_minecraft")
            )
        })
    })
}

/// The account's UUID (without dashes) and name.
pub fn profile(agent: &ureq::Agent, mc_token: &str) -> Result<(String, String)> {
    let (status, text) = body(
        agent
            .get(&format!("{SERVICES}/minecraft/profile"))
            .set("Authorization", &format!("Bearer {mc_token}"))
            .call(),
        "Minecraft",
    )?;
    if status == 404 {
        bail!("This account has no Minecraft profile yet. Create one on minecraft.net first");
    }
    if status != 200 {
        bail!("Minecraft could not read this account's profile (HTTP {status})");
    }
    #[derive(Deserialize)]
    struct Profile {
        id: String,
        name: String,
    }
    let profile: Profile = parse(&text, "Minecraft")?;
    Ok((profile.id, profile.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_includes_game_pass() {
        let owned = json!({"items": [{"name": "product_minecraft"}, {"name": "game_minecraft"}]});
        let pass = json!({"items": [{"name": "product_minecraft"}]});
        let none = json!({"items": [{"name": "product_dungeons"}]});
        assert!(owns_from(&owned));
        assert!(owns_from(&pass));
        assert!(!owns_from(&none));
        assert!(!owns_from(&json!({})));
    }

    #[test]
    fn xbox_errors_become_readable() {
        assert!(xsts_error(Some(2148916233), 401).contains("no Xbox profile"));
        assert!(xsts_error(Some(2148916238), 401).contains("child account"));
        assert!(xsts_error(Some(1), 401).contains("error 1"));
        assert!(xsts_error(None, 500).contains("HTTP 500"));
    }

    #[test]
    fn misconfigured_app_is_explained() {
        let e = OAuthError {
            error: "unauthorized_client".into(),
            error_description: "AADSTS700016: Application not found. Trace ID: abc".into(),
        };
        let error = app_error(&e);
        assert_eq!(
            error.to_string(),
            "Microsoft refused this launcher's sign-in app (AADSTS700016). It must allow \
             personal Microsoft accounts and public client flows"
        );
        let full = format!("{error:#}");
        assert!(full.contains("AADSTS700016: Application not found."));
        assert!(!full.contains("Trace ID"));
    }

    #[test]
    fn reads_a_device_code() {
        let code: DeviceCode = serde_json::from_str(
            r#"{"device_code":"d","user_code":"ABCD1234","verification_uri":"https://www.microsoft.com/link","expires_in":900}"#,
        )
        .unwrap();
        assert_eq!(code.interval, 5);
        assert_eq!(code.user_code, "ABCD1234");
    }

    #[test]
    fn the_client_id_can_be_replaced() {
        // Only this test sets the variable.
        assert_eq!(client_id(), CLIENT_ID);
        std::env::set_var(CLIENT_ID_ENV, "other");
        assert_eq!(client_id(), "other");
        std::env::remove_var(CLIENT_ID_ENV);
    }
}
