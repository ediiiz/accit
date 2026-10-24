//! Fail-closed loading (docs/spec.md section 12): production needs its Portal, proxy trust and database settings.

mod common;

use common::{host, parse_with_file};

const PRODUCTION: &str = "
profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
protection:
  mode: all
";

fn without(key: &str) -> String {
    PRODUCTION.lines().filter(|line| !line.starts_with(key)).collect::<Vec<_>>().join("\n")
}

fn load(yaml: &str) -> anyhow::Result<config::Config> {
    parse_with_file(yaml, "token")
}

#[test]
fn a_complete_production_config_loads() {
    let config = load(PRODUCTION).unwrap();

    assert_eq!(config.portal_host, Some(host("access.example.com")));
}

#[test]
fn production_requires_public_origin() {
    assert!(load(&without("public_origin")).is_err());
}

#[test]
fn production_requires_trusted_proxy() {
    assert!(load(&without("trusted_proxy")).is_err());
}

#[test]
fn production_requires_proxy_token_file() {
    assert!(load(&without("proxy_token_file")).is_err());
}

#[test]
fn production_requires_database() {
    assert!(load(&without("database")).is_err());
}

#[test]
fn the_error_names_the_missing_key() {
    let message = format!("{:#}", load(&without("trusted_proxy")).unwrap_err());

    assert!(message.contains("trusted_proxy"), "{message}");
}

#[test]
fn the_test_profile_may_omit_them() {
    assert!(parse_with_file("profile: test\nprotection:\n  mode: all\n", "").is_ok());
}

#[test]
fn public_origin_is_an_https_origin() {
    let origin = |value: &str| load(&PRODUCTION.replace("https://access.example.com", value));

    for value in [
        "https://access.example.com",
        "https://access.example.com/",
        "https://access.example.com:8443",
        "https://Access.Example.com:8443/",
    ] {
        assert!(origin(value).is_ok(), "{value}");
    }

    for value in [
        "not a url",
        "access.example.com",
        "http://access.example.com",
        "https://",
        "https:///",
        "https://access.example.com/path",
        "'https://access.example.com?x'",
        "'https://access.example.com#x'",
        "https://user@access.example.com",
        "https://access.example.com:",
        "https://access.example.com:99999",
        "https://10.0.0.1",
        "'https://*.example.com'",
    ] {
        assert!(origin(value).is_err(), "{value}");
    }
}

#[test]
fn the_portal_host_is_the_origins_hostname() {
    let config =
        load(&PRODUCTION.replace("https://access.example.com", "https://Access.Example.com:8443/")).unwrap();

    assert_eq!(config.portal_host, Some(host("access.example.com")));
}

#[test]
fn production_rejects_the_portal_host_in_the_policy() {
    let yaml = PRODUCTION.replace("mode: all", "mode: all\n  excluded_hosts: [access.example.com]");

    assert!(load(&yaml).is_err());
}
