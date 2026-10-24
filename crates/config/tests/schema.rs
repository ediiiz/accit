//! The configuration schema of docs/spec.md section 12: keys, required values, defaults.

mod common;

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use access::Policy;
use common::{host, parse, parse_with_file, with};
use config::{Config, Language, Profile};

const FULL: &str = "
profile: production
public_origin: https://access.example.com
portal_host: access.example.com
title: Home access
default_language: de
grant_duration: 3d
max_grants: 50
audit_retention: 14d
apps:
  - name: Jellyfin
    url: https://jellyfin.example.com
protection:
  mode: all
  excluded_hosts:
    - status.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
client_ca_file: /config/client-ca.pem
listen:
  portal: 127.0.0.1:9080
  verify: 127.0.0.1:9081
database: /data/accit.db
admin_socket: /data/admin.sock
";

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn full() -> Config {
    parse_with_file(FULL, "token\n").unwrap()
}

#[test]
fn every_key_parses_identity_and_limits() {
    let config = full();

    assert_eq!(config.profile, Profile::Production);
    assert_eq!(config.public_origin.as_deref(), Some("https://access.example.com"));
    assert_eq!(config.portal_host, Some(host("access.example.com")));
    assert_eq!(config.title, "Home access");
    assert_eq!(config.default_language, Language::De);
    assert_eq!(config.grant_duration, Duration::from_hours(72));
    assert_eq!(config.max_grants, 50);
    assert_eq!(config.audit_retention, Duration::from_hours(336));
}

#[test]
fn every_key_parses_policy_and_proxy() {
    let config = full();

    assert_eq!(config.apps.len(), 1);
    assert_eq!(config.protection, Policy::All { excluded: [host("status.example.com")].into() });
    assert_eq!(config.trusted_proxy, Some(IpAddr::V4(Ipv4Addr::new(172, 30, 0, 2))));
    assert_eq!(config.proxy_token_file.as_deref(), Some("/secrets/proxy-token".as_ref()));
    assert_eq!(config.proxy_token.unwrap().expose(), "token");
}

#[test]
fn every_key_parses_paths_and_listeners() {
    let config = full();

    assert_eq!(config.client_ca_file.as_deref(), Some("/config/client-ca.pem".as_ref()));
    assert_eq!(config.listen.portal, "127.0.0.1:9080".parse().unwrap());
    assert_eq!(config.listen.verify, "127.0.0.1:9081".parse().unwrap());
    assert_eq!(config.database.as_deref(), Some("/data/accit.db".as_ref()));
    assert_eq!(config.admin_socket.as_deref(), Some("/data/admin.sock".as_ref()));
}

#[test]
fn profile_has_no_default() {
    assert!(parse("protection:\n  mode: selected\n").is_err());
}

#[test]
fn protection_mode_has_no_default() {
    assert!(parse("profile: test\n").is_err());
    assert!(parse("profile: test\nprotection: {}\n").is_err());
    assert!(parse("profile: test\nprotection:\n  hosts: [a.example.com]\n").is_err());
}

#[test]
fn unknown_profile_and_mode_are_rejected() {
    assert!(parse("profile: staging\nprotection:\n  mode: selected\n").is_err());
    assert!(parse("profile: test\nprotection:\n  mode: some\n").is_err());
}

#[test]
fn unknown_keys_are_rejected_everywhere() {
    assert!(with("profiel: production\n").is_err());
    assert!(with("listen:\n  admin: 0.0.0.0:9000\n").is_err());
    assert!(parse("profile: test\nprotection:\n  mode: selected\n  hostz: []\n").is_err());
    assert!(with("apps:\n  - name: A\n    url: https://a.example.com\n    icon: a.png\n").is_err());
}

#[test]
fn trusted_networks_wait_for_their_own_ticket() {
    assert!(with("trusted_networks: [192.168.1.0/24]\n").is_err());
    assert!(with("docker_networks: [172.30.0.0/24]\n").is_err());
}

#[test]
fn optional_keys_default_to_absent_or_documented_values() {
    let config = with("").unwrap();

    assert_eq!(config.title, "Access");
    assert_eq!(config.default_language, Language::En);
    assert!(config.apps.is_empty());
    assert!(config.public_origin.is_none() && config.portal_host.is_none() && config.trusted_proxy.is_none());
    assert!(config.proxy_token_file.is_none() && config.client_ca_file.is_none());
    assert!(config.database.is_none() && config.admin_socket.is_none());
    assert_eq!(config.listen.portal.port(), 8080);
    assert_eq!(config.listen.verify.port(), 8081);
}

#[test]
fn unknown_language_is_rejected() {
    assert!(with("default_language: fr\n").is_err());
}

#[test]
fn trusted_proxy_is_an_exact_address() {
    assert!(with("trusted_proxy: 172.30.0.0/24\n").is_err());
    assert!(with("trusted_proxy: traefik\n").is_err());
}
