//! `accit admin check [file]` (docs/spec.md section 11): loads the file as startup does and
//! reports what it would cover. Never touches the running daemon.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use access::{Gate, Hostname, Policy};
use anyhow::{Context, Result};
use config::Config;

use crate::client_ca;
use crate::verify::GATES_NOTHING;

const DAY: i64 = 86_400;
const WARN_WITHIN_DAYS: i64 = 30;

#[derive(Debug)]
pub struct Report {
    pub text: String,
    pub failed: bool,
}

/// What the gate does without a Grant, as `verify` decides it.
fn is_gated(config: &Config, host: &Hostname) -> bool {
    Gate {
        policy: &config.protection,
        trusted: &config.trusted_networks,
        portal_host: config.portal_host.as_ref(),
        public_only: false,
    }
    .is_gated(host)
}

/// Every host the config names: the Policy's list, app links and the Portal.
fn known_hosts(config: &Config) -> BTreeSet<&Hostname> {
    config
        .protection
        .listed()
        .chain(config.apps.iter().map(|app| &app.host))
        .chain(config.portal_host.as_ref())
        .collect()
}

fn state(gated: bool) -> &'static str {
    if gated { "gated" } else { "open" }
}

fn coverage(config: &Config) -> String {
    let mut text = String::new();

    for host in known_hosts(config) {
        let _ = writeln!(text, "{host}  {}", state(is_gated(config, host)));
    }

    if matches!(config.protection, Policy::All { .. }) {
        let _ = writeln!(text, "other hosts  gated");
    }

    text
}

fn warnings(config: &Config) -> Vec<String> {
    let empty = config.protection.gates_nothing().then(|| GATES_NOTHING.to_owned());
    let open_links = config
        .apps
        .iter()
        .filter(|app| !is_gated(config, &app.host))
        .map(|app| format!("app link `{}` points to open host {}", app.name, app.host));

    empty.into_iter().chain(open_links).collect()
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
}

/// The client CA line, and whether it is an error. Nothing without a `client_ca_file`.
fn client_ca(file: &Path, now: SystemTime) -> (String, bool) {
    let expiry = std::fs::read(file)
        .context("unreadable")
        .and_then(|pem| client_ca::earliest_expiry(&pem))
        .map(|expiry| expiry - unix_seconds(now));

    match expiry {
        Ok(left) if left <= 0 => ("error: client CA expired".to_owned(), true),
        Ok(left) if left <= WARN_WITHIN_DAYS * DAY => {
            (format!("warning: client CA: {} days to expiry", left / DAY), false)
        }
        Ok(left) => (format!("client CA: {} days to expiry", left / DAY), false),
        Err(error) => (format!("error: client CA {}: {error:#}", file.display()), true),
    }
}

pub fn run(path: &Path, now: SystemTime) -> Result<Report> {
    let config = Config::load(path)?;
    let mut text = coverage(&config);

    for warning in warnings(&config) {
        let _ = writeln!(text, "warning: {warning}");
    }

    let ca = config.client_ca_file.as_deref().map(|file| client_ca(file, now));
    let failed = ca.as_ref().is_some_and(|(_, failed)| *failed);

    if let Some((line, _)) = ca {
        let _ = writeln!(text, "{line}");
    }

    Ok(Report { text, failed })
}
