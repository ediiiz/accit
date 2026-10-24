//! Startup warnings: the `test` profile names each skipped proxy check (docs/spec.md section 3);
//! a Policy that gates nothing is reported loudly (section 4).

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use common::{config, grants, production, trust};
use server::trust::ProxyTrust;
use server::verify::Verifier;
use tracing_subscriber::fmt::MakeWriter;

const TEST_PROFILE: &str = "profile: test\nprotection:\n  mode: selected\n";
const SKIPPABLE: [&str; 3] = ["trusted_proxy", "proxy_token_file", "public_origin"];

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().map_err(|_| io::Error::other("poisoned"))?.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for Captured {
    type Writer = Self;

    fn make_writer(&self) -> Self {
        self.clone()
    }
}

fn log_of(trust: &ProxyTrust) -> String {
    captured(|| trust.log_startup())
}

fn captured(log: impl FnOnce()) -> String {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt().with_writer(captured.clone()).with_ansi(false).finish();

    tracing::subscriber::with_default(subscriber, log);

    let bytes = captured.0.lock().map(|bytes| bytes.clone()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn startup_log(yaml: &str) -> String {
    log_of(&trust(yaml))
}

fn named(log: &str) -> Vec<&'static str> {
    SKIPPABLE.into_iter().filter(|key| log.contains(key)).collect()
}

#[test]
fn a_bare_test_profile_warns_about_every_skipped_check() {
    let log = startup_log(TEST_PROFILE);

    assert!(log.contains("WARN"), "{log}");
    assert_eq!(named(&log), SKIPPABLE, "{log}");
}

#[test]
fn only_the_checks_still_missing_are_named() {
    let pinned = format!("{TEST_PROFILE}trusted_proxy: 172.30.0.2\n");
    let log = startup_log(&pinned);

    assert_eq!(named(&log), ["proxy_token_file", "public_origin"], "{log}");

    let all_but_origin = format!("{TEST_PROFILE}trusted_proxy: 172.30.0.2\nproxy_token_file: /t\n");

    assert_eq!(named(&startup_log(&all_but_origin)), ["public_origin"]);
}

#[test]
fn a_fully_configured_test_profile_does_not_warn() {
    let complete = format!(
        "{TEST_PROFILE}trusted_proxy: 172.30.0.2\nproxy_token_file: /t\npublic_origin: https://access.example.com\n"
    );

    assert_eq!(startup_log(&complete), "");
}

#[test]
fn production_does_not_warn() {
    assert_eq!(log_of(&production()), "");
}

async fn policy_log(protection: &str) -> String {
    let yaml = format!("profile: test\nprotection:\n{protection}");
    let verifier = Verifier::new(&config(&yaml), grants().await);

    captured(|| verifier.log_startup())
}

#[tokio::test]
async fn an_empty_selection_is_reported_loudly() {
    let log = policy_log("  mode: selected\n").await;

    assert!(log.contains("WARN"), "{log}");
    assert!(log.contains("gates nothing"), "{log}");
}

#[tokio::test]
async fn a_policy_that_gates_something_is_not_reported() {
    assert_eq!(policy_log("  mode: selected\n  hosts: [jellyfin.example.com]\n").await, "");
    assert_eq!(policy_log("  mode: all\n").await, "");
}
