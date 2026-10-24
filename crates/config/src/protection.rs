//! `protection:` into an [`access::Policy`] (docs/spec.md section 4).

use std::collections::BTreeSet;

use access::{Hostname, Policy};
use anyhow::{Context, Result, bail};
use serde::Deserialize;

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
    hosts: Option<Vec<String>>,
    excluded_hosts: Option<Vec<String>>,
}

fn hostnames(key: &str, entries: &[String]) -> Result<BTreeSet<Hostname>> {
    entries
        .iter()
        .map(|entry| Hostname::parse(entry).with_context(|| format!("protection.{key}: `{entry}`")))
        .collect()
}

impl RawProtection {
    pub(crate) fn validate(self, portal_host: Option<&Hostname>) -> Result<Policy> {
        let policy = match (self.mode, self.hosts, self.excluded_hosts) {
            (Mode::Selected, hosts, None) => {
                Policy::Selected { hosts: hostnames("hosts", &hosts.unwrap_or_default())? }
            }
            (Mode::All, None, excluded) => {
                Policy::All { excluded: hostnames("excluded_hosts", &excluded.unwrap_or_default())? }
            }
            (Mode::Selected, _, Some(_)) => bail!("protection.excluded_hosts is only allowed in all mode"),
            (Mode::All, Some(_), _) => bail!("protection.hosts is only allowed in selected mode"),
        };

        if let Some(portal) = portal_host.filter(|portal| policy.listed().any(|listed| listed == *portal)) {
            bail!("the Portal host `{portal}` may not appear in protection");
        }

        Ok(policy)
    }
}
