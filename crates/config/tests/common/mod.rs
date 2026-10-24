//! Shared fixtures: configuration parsing with a reader that never touches the disk.

#![allow(dead_code, reason = "each test binary uses a different subset")]

use std::io;
use std::path::Path;

use access::Hostname;
use config::Config;

pub const SELECTED: &str = "profile: test\nprotection:\n  mode: selected\n";

pub fn parse(yaml: &str) -> anyhow::Result<Config> {
    Config::parse(yaml, |_| Err(io::ErrorKind::NotFound.into()))
}

pub fn parse_with_file(yaml: &str, contents: &'static str) -> anyhow::Result<Config> {
    Config::parse(yaml, |_: &Path| Ok(contents.to_owned()))
}

/// Accepted config with `extra` appended to the minimal one.
pub fn with(extra: &str) -> anyhow::Result<Config> {
    parse(&format!("{SELECTED}{extra}"))
}

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
pub fn host(name: &str) -> Hostname {
    Hostname::parse(name).unwrap()
}
