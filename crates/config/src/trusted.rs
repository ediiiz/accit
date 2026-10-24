//! `trusted_networks`, `trusted_networks_exclude` and `docker_networks` (docs/spec.md section 5).

use access::{Address, Cidr, TrustedNetworks};
use anyhow::{Context, Result};

fn parse_all<T>(key: &str, entries: &[String]) -> Result<Vec<T>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    entries.iter().map(|entry| entry.parse().with_context(|| format!("{key}: `{entry}`"))).collect()
}

pub(crate) fn docker_networks(entries: &[String]) -> Result<Vec<Cidr>> {
    parse_all("docker_networks", entries)
}

pub(crate) fn networks(trusted: &[String], excluded: &[String], docker: &[Cidr]) -> Result<TrustedNetworks> {
    let trusted: Vec<Cidr> = parse_all("protection.trusted_networks", trusted)?;
    let excluded: Vec<Address> = parse_all("protection.trusted_networks_exclude", excluded)?;

    TrustedNetworks::new(trusted, excluded, docker).context("protection.trusted_networks")
}
