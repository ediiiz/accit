use std::io;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::Duration;

use access::{Hostname, Policy, TrustedNetworks};
use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::apps::{AppLink, RawApp};
use crate::duration;
use crate::listen::Listen;
use crate::origin;
use crate::protection::RawProtection;
use crate::token::ProxyToken;
use crate::trusted;

const GRANT_DURATION_MIN: Duration = Duration::from_hours(1);
const GRANT_DURATION_MAX: Duration = Duration::from_hours(30 * 24);
const AUDIT_RETENTION_MIN: Duration = Duration::from_hours(1);
const SEVEN_DAYS: &str = "7d";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Production,
    Test,
}

/// Portal languages; the Portal picks the browser's language and falls back to this.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    En,
    #[serde(rename = "de")]
    De,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub profile: Profile,
    pub public_origin: Option<String>,
    pub portal_host: Option<Hostname>,
    pub title: String,
    pub default_language: Language,
    pub grant_duration: Duration,
    pub max_grants: u32,
    pub audit_retention: Duration,
    pub apps: Vec<AppLink>,
    pub protection: Policy,
    pub trusted_networks: TrustedNetworks,
    pub trusted_proxy: Option<IpAddr>,
    pub proxy_token_file: Option<PathBuf>,
    pub proxy_token: Option<ProxyToken>,
    pub client_ca_file: Option<PathBuf>,
    pub listen: Listen,
    pub database: Option<PathBuf>,
    pub admin_socket: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    /// Required on purpose: `test` allows private Addresses, so it must never be a default.
    profile: Profile,
    public_origin: Option<String>,
    #[serde(default = "default_title")]
    title: String,
    #[serde(default)]
    default_language: Language,
    #[serde(default = "default_duration")]
    grant_duration: String,
    #[serde(default = "default_max_grants")]
    max_grants: NonZeroU32,
    #[serde(default = "default_duration")]
    audit_retention: String,
    #[serde(default)]
    apps: Vec<RawApp>,
    protection: RawProtection,
    trusted_proxy: Option<IpAddr>,
    proxy_token_file: Option<PathBuf>,
    #[serde(default)]
    docker_networks: Vec<String>,
    client_ca_file: Option<PathBuf>,
    #[serde(default)]
    listen: Listen,
    database: Option<PathBuf>,
    admin_socket: Option<PathBuf>,
}

fn default_title() -> String {
    "Access".to_owned()
}

fn default_duration() -> String {
    SEVEN_DAYS.to_owned()
}

fn default_max_grants() -> NonZeroU32 {
    NonZeroU32::MIN.saturating_add(999)
}

fn read_token(path: &Path, read_file: impl FnOnce(&Path) -> io::Result<String>) -> Result<ProxyToken> {
    let contents = read_file(path).with_context(|| format!("reading proxy_token_file {}", path.display()))?;
    let value = contents.trim();

    if value.is_empty() {
        bail!("proxy_token_file {} is empty", path.display());
    }

    Ok(ProxyToken::new(value.to_owned()))
}

fn grant_duration(text: &str) -> Result<Duration> {
    let value = duration::parse(text).context("grant_duration")?;

    if !(GRANT_DURATION_MIN..=GRANT_DURATION_MAX).contains(&value) {
        bail!("grant_duration must be between 1h and 30d, got `{text}`");
    }

    Ok(value)
}

fn audit_retention(text: &str) -> Result<Duration> {
    let value = duration::parse(text).context("audit_retention")?;

    if value < AUDIT_RETENTION_MIN {
        bail!("audit_retention must be at least 1h, got `{text}`");
    }

    Ok(value)
}

/// Production fails closed: no Portal host or proxy trust means no way to tell who is calling.
fn require_production_keys(raw: &Raw) -> Result<()> {
    let missing = [
        ("public_origin", raw.public_origin.is_none()),
        ("trusted_proxy", raw.trusted_proxy.is_none()),
        ("proxy_token_file", raw.proxy_token_file.is_none()),
        ("database", raw.database.is_none()),
    ];

    if let Some((key, _)) = missing.iter().find(|(_, absent)| *absent) {
        bail!("profile production requires `{key}`");
    }

    Ok(())
}

impl Config {
    /// Parses and validates `yaml`; `read_file` supplies the contents of `proxy_token_file`.
    pub fn parse(yaml: &str, read_file: impl FnOnce(&Path) -> io::Result<String>) -> Result<Self> {
        let raw: Raw = serde_saphyr::from_str(yaml).context("invalid configuration")?;

        if raw.profile == Profile::Production {
            require_production_keys(&raw)?;
        }

        let (public_origin, portal_host) = match raw.public_origin.as_deref().map(origin::public_origin) {
            Some(parsed) => {
                let (origin, host) = parsed?;
                (Some(origin), Some(host))
            }
            None => (None, None),
        };
        let proxy_token =
            raw.proxy_token_file.as_deref().map(|path| read_token(path, read_file)).transpose()?;
        let docker_networks = trusted::docker_networks(&raw.docker_networks)?;
        let (protection, trusted_networks) =
            raw.protection.validate(portal_host.as_ref(), &docker_networks)?;

        Ok(Self {
            profile: raw.profile,
            public_origin,
            title: raw.title,
            default_language: raw.default_language,
            grant_duration: grant_duration(&raw.grant_duration)?,
            max_grants: raw.max_grants.get(),
            audit_retention: audit_retention(&raw.audit_retention)?,
            apps: raw.apps.into_iter().map(RawApp::validate).collect::<Result<_>>()?,
            protection,
            trusted_networks,
            portal_host,
            trusted_proxy: raw.trusted_proxy.map(|address| address.to_canonical()),
            proxy_token_file: raw.proxy_token_file,
            proxy_token,
            client_ca_file: raw.client_ca_file,
            listen: raw.listen,
            database: raw.database,
            admin_socket: raw.admin_socket,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        Self::parse(&text, |file| std::fs::read_to_string(file))
            .with_context(|| format!("in {}", path.display()))
    }
}
