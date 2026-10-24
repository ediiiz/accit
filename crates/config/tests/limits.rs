//! App links, Grant duration, Grant cap and audit retention (docs/spec.md sections 6, 7 and 9).

mod common;

use std::time::Duration;

use common::{parse, with};

const HOUR: u64 = 3_600;
const DAY: u64 = 86_400;

fn app(url: &str) -> String {
    format!("apps:\n  - name: Jellyfin\n    url: {url}\n")
}

#[test]
fn app_links_need_https() {
    let config = with(&app("https://jellyfin.example.com")).unwrap();

    assert_eq!(config.apps[0].name, "Jellyfin");
    assert_eq!(config.apps[0].url, "https://jellyfin.example.com");
}

#[test]
fn app_links_reject_other_schemes() {
    for url in [
        "http://jellyfin.example.com",
        "ftp://x.example.com",
        "'javascript:alert(1)'",
        "jellyfin.example.com",
    ] {
        assert!(with(&app(url)).is_err(), "{url}");
    }
}

#[test]
fn app_links_without_a_host_are_rejected() {
    assert!(with(&app("'https://'")).is_err());
}

#[test]
fn app_links_need_a_name() {
    assert!(with("apps:\n  - url: https://a.example.com\n").is_err());
    assert!(with("apps:\n  - name: ''\n    url: https://a.example.com\n").is_err());
    assert!(with("apps:\n  - name: '  '\n    url: https://a.example.com\n").is_err());
}

#[test]
fn app_links_need_a_url() {
    assert!(with("apps:\n  - name: Jellyfin\n").is_err());
}

#[test]
fn defaults_are_seven_days_one_thousand_grants_seven_days_of_audit() {
    let config = with("").unwrap();

    assert_eq!(config.grant_duration, Duration::from_secs(7 * DAY));
    assert_eq!(config.max_grants, 1_000);
    assert_eq!(config.audit_retention, Duration::from_secs(7 * DAY));
}

#[test]
fn grant_duration_runs_from_one_hour_to_thirty_days() {
    assert_eq!(with("grant_duration: 1h\n").unwrap().grant_duration, Duration::from_secs(HOUR));
    assert_eq!(with("grant_duration: 30d\n").unwrap().grant_duration, Duration::from_secs(30 * DAY));
    assert_eq!(with("grant_duration: 90m\n").unwrap().grant_duration, Duration::from_mins(90));
}

#[test]
fn grant_duration_outside_the_range_is_rejected() {
    for value in ["59m", "3599s", "0d", "31d", "721h"] {
        assert!(with(&format!("grant_duration: {value}\n")).is_err(), "{value}");
    }
}

#[test]
fn malformed_durations_are_rejected() {
    for value in ["soon", "7", "d", "-1d", "1.5d", "7 days", "''"] {
        assert!(with(&format!("grant_duration: {value}\n")).is_err(), "grant_duration {value}");
        assert!(with(&format!("audit_retention: {value}\n")).is_err(), "audit_retention {value}");
    }
}

#[test]
fn audit_retention_and_cap_are_configurable() {
    let config = with("audit_retention: 30d\nmax_grants: 25\n").unwrap();

    assert_eq!(config.audit_retention, Duration::from_secs(30 * DAY));
    assert_eq!(config.max_grants, 25);
}

#[test]
fn a_negative_cap_is_rejected() {
    assert!(with("max_grants: -1\n").is_err());
    assert!(parse("profile: test\nprotection:\n  mode: selected\nmax_grants: many\n").is_err());
}
