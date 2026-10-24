//! `protection:` into an [`access::Policy`] and [`access::TrustedNetworks`] (docs/spec.md sections 4 and 5).

use std::collections::BTreeSet;

use access::{Cidr, Hostname, Policy, TrustedNetworks};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer};

use crate::trusted;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Selected,
    All,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProtection {
    /// Required on purpose: no mode is a safe default.
    mode: Mode,
    #[serde(default)]
    hosts: Key,
    #[serde(default)]
    excluded_hosts: Key,
    #[serde(default)]
    trusted_networks: Option<Vec<String>>,
    #[serde(default)]
    trusted_networks_exclude: Option<Vec<String>>,
}

/// A list key as written; `Null` (`hosts:`) is kept apart from `Absent` so the wrong mode can reject it.
#[derive(Debug, Default)]
enum Key {
    #[default]
    Absent,
    Null,
    List(Vec<String>),
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Option::deserialize(deserializer)?.map_or(Self::Null, Self::List))
    }
}

impl Key {
    fn entries(&self) -> &[String] {
        match self {
            Self::List(entries) => entries,
            Self::Absent | Self::Null => &[],
        }
    }
}

fn hostnames(key: &str, entries: &[String]) -> Result<BTreeSet<Hostname>> {
    entries
        .iter()
        .map(|entry| Hostname::parse(entry).with_context(|| format!("protection.{key}: `{entry}`")))
        .collect()
}

impl RawProtection {
    pub(crate) fn validate(
        self,
        portal_host: Option<&Hostname>,
        docker_networks: &[Cidr],
    ) -> Result<(Policy, TrustedNetworks)> {
        let policy = match (self.mode, &self.hosts, &self.excluded_hosts) {
            (Mode::Selected, hosts, Key::Absent) => {
                Policy::Selected { hosts: hostnames("hosts", hosts.entries())? }
            }
            (Mode::All, Key::Absent, excluded) => {
                Policy::All { excluded: hostnames("excluded_hosts", excluded.entries())? }
            }
            (Mode::Selected, _, _) => bail!("protection.excluded_hosts is only allowed in all mode"),
            (Mode::All, _, _) => bail!("protection.hosts is only allowed in selected mode"),
        };

        if let Some(portal) = portal_host.filter(|portal| policy.listed().any(|listed| listed == *portal)) {
            bail!("the Portal host `{portal}` may not appear in protection");
        }

        let trusted = trusted::networks(
            self.trusted_networks.as_deref().unwrap_or_default(),
            self.trusted_networks_exclude.as_deref().unwrap_or_default(),
            docker_networks,
        )?;

        Ok((policy, trusted))
    }
}
