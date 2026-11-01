//! Server addresses for "Join a server": checked here, so nothing odd reaches a game's
//! command line.

use anyhow::{bail, Result};

/// Bedrock's default port; Java's (25565) is the game's own default.
pub const BEDROCK_PORT: u16 = 19132;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub host: String,
    pub port: Option<u16>,
}

impl Address {
    /// `host`, `host:port`, `[v6]` or `[v6]:port`.
    pub fn parse(input: &str) -> Result<Address> {
        let input = input.trim();
        if input.is_empty() {
            bail!("Enter a server address");
        }
        let (host, port) = if let Some(rest) = input.strip_prefix('[') {
            let Some((host, after)) = rest.split_once(']') else {
                bail!("That server address is not valid");
            };
            let port = match after {
                "" => None,
                p => Some(p.strip_prefix(':').unwrap_or("x")),
            };
            (format!("[{host}]"), port)
        } else {
            match input.rsplit_once(':') {
                Some((host, port)) if !host.contains(':') => (host.to_string(), Some(port)),
                Some(_) => bail!("Put an IPv6 address in brackets, like [::1]:25565"),
                None => (input.to_string(), None),
            }
        };
        let plain = host.trim_start_matches('[').trim_end_matches(']');
        // A host starting with `-` could be read as an option by the game.
        if plain.is_empty()
            || plain.starts_with('-')
            || host.len() > 255
            || !plain
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':'))
        {
            bail!("That server address is not valid");
        }
        let port = match port {
            None => None,
            Some(p) => match p.parse::<u16>() {
                Ok(p) if p > 0 => Some(p),
                _ => bail!("The port must be a number from 1 to 65535"),
            },
        };
        Ok(Address { host, port })
    }

    /// For Horizon: `host` or `host:port`.
    pub fn java(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{port}", self.host),
            None => self.host.clone(),
        }
    }

    /// For Cinnabar, which needs the port: `host:port`, 19132 by default.
    pub fn bedrock(&self) -> String {
        format!("{}:{}", self.host, self.port.unwrap_or(BEDROCK_PORT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_addresses() {
        let a = Address::parse(" play.example.net ").unwrap();
        assert_eq!(a.java(), "play.example.net");
        assert_eq!(a.bedrock(), "play.example.net:19132");
        let b = Address::parse("1.2.3.4:25570").unwrap();
        assert_eq!(b.java(), "1.2.3.4:25570");
        assert_eq!(b.bedrock(), "1.2.3.4:25570");
        let v6 = Address::parse("[::1]:19133").unwrap();
        assert_eq!(v6.bedrock(), "[::1]:19133");
        assert_eq!(Address::parse("[::1]").unwrap().java(), "[::1]");
    }

    #[test]
    fn refuses_bad_addresses() {
        for bad in [
            "",
            "a b",
            "host:0",
            "host:99999",
            "host:x",
            "::1",
            "--fullscreen;rm",
            "--fullscreen",
            "-host:25565",
            "[::1",
        ] {
            assert!(Address::parse(bad).is_err(), "{bad:?} should be refused");
        }
    }
}
