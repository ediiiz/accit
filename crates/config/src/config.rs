use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use access::{Hostname, Policy};
use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::apps::{AppLink, RawApp};
use crate::duration;
use crate::listen::Listen;
use crate::protection::RawProtection;
use crate::token::ProxyToken;

const GRANT_DURATION_MIN: Duration = Duration::from_hours(1);
const GRANT_DURATION_MAX: Duration = Duration::from_hours(30 * 24);
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
    portal_host: Option<String>,
    #[serde(default = "default_title")]
    title: String,
    #[serde(default)]
    default_language: Language,
    #[serde(default = "default_duration")]
    grant_duration: String,
    #[serde(default = "default_max_grants")]
    max_grants: u32,
    #[serde(default = "default_duration")]
    audit_retention: String,
    #[serde(default)]
    apps: Vec<RawApp>,
    protection: RawProtection,
    trusted_proxy: Option<IpAddr>,
    proxy_token_file: Option<PathBuf>,
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

fn default_max_grants() -> u32 {
    1_000
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

impl Config {
    /// Parses and validates `yaml`; `read_file` supplies the contents of `proxy_token_file`.
    pub fn parse(yaml: &str, read_file: impl FnOnce(&Path) -> io::Result<String>) -> Result<Self> {
        let raw: Raw = serde_saphyr::from_str(yaml).context("invalid configuration")?;
        let portal_host =
            raw.portal_host.as_deref().map(Hostname::parse).transpose().context("portal_host")?;
        let proxy_token =
            raw.proxy_token_file.as_deref().map(|path| read_token(path, read_file)).transpose()?;

        Ok(Self {
            profile: raw.profile,
            public_origin: raw.public_origin,
            title: raw.title,
            default_language: raw.default_language,
            grant_duration: grant_duration(&raw.grant_duration)?,
            max_grants: raw.max_grants,
            audit_retention: duration::parse(&raw.audit_retention).context("audit_retention")?,
            apps: raw.apps.into_iter().map(RawApp::validate).collect::<Result<_>>()?,
            protection: raw.protection.validate(portal_host.as_ref())?,
            portal_host,
            trusted_proxy: raw.trusted_proxy,
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
