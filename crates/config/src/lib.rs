//! accit configuration (`/config/accit.yaml`), read at startup and on reload. Unknown keys are errors.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

pub const DEFAULT_PATH: &str = "/config/accit.yaml";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Required on purpose: `test` allows private Addresses, so it must never be a default.
    pub profile: Profile,
    #[serde(default)]
    pub listen: Listen,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Production,
    Test,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Listen {
    #[serde(default = "Listen::default_portal")]
    pub portal: SocketAddr,
    #[serde(default = "Listen::default_verify")]
    pub verify: SocketAddr,
}

impl Listen {
    fn default_portal() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080))
    }

    fn default_verify() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8081))
    }
}

impl Default for Listen {
    fn default() -> Self {
        Self { portal: Self::default_portal(), verify: Self::default_verify() }
    }
}

impl Config {
    pub fn parse(yaml: &str) -> Result<Self> {
        serde_saphyr::from_str(yaml).context("invalid configuration")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        Self::parse(&text).with_context(|| format!("in {}", path.display()))
    }
}

/// `ACCIT_CONFIG`, else [`DEFAULT_PATH`].
pub fn path_from_env() -> PathBuf {
    std::env::var_os("ACCIT_CONFIG").map_or_else(|| PathBuf::from(DEFAULT_PATH), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_required() {
        assert!(Config::parse("listen: {}\n").is_err());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("profile: test\nprofiel: production\n").is_err());
        assert!(Config::parse("profile: test\nlisten:\n  admin: 0.0.0.0:9000\n").is_err());
    }

    #[test]
    fn unknown_profile_is_rejected() {
        assert!(Config::parse("profile: staging\n").is_err());
    }

    #[test]
    fn listeners_default_to_8080_and_8081() {
        let config = Config::parse("profile: production\n").unwrap();

        assert_eq!(config.profile, Profile::Production);
        assert_eq!(config.listen, Listen::default());
        assert_eq!(config.listen.portal.port(), 8080);
        assert_eq!(config.listen.verify.port(), 8081);
    }

    #[test]
    fn listeners_can_be_moved() {
        let config = Config::parse("profile: test\nlisten:\n  verify: 127.0.0.1:9081\n").unwrap();

        assert_eq!(config.listen.verify, "127.0.0.1:9081".parse().unwrap());
        assert_eq!(config.listen.portal.port(), 8080);
    }
}
