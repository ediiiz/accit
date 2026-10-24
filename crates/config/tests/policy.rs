//! Policy rules of docs/spec.md section 4.

mod common;

use access::Policy;
use common::{host, parse};

fn selected(hosts: &str) -> String {
    format!("profile: test\nprotection:\n  mode: selected\n  hosts: {hosts}\n")
}

fn all(excluded: &str) -> String {
    format!("profile: test\nprotection:\n  mode: all\n  excluded_hosts: {excluded}\n")
}

#[test]
fn selected_mode_lists_hosts_canonicalized_and_deduplicated() {
    let config =
        parse(&selected("[Jellyfin.Example.com, jellyfin.example.com., photos.example.com]")).unwrap();

    assert_eq!(
        config.protection,
        Policy::Selected { hosts: [host("jellyfin.example.com"), host("photos.example.com")].into() }
    );
}

#[test]
fn all_mode_lists_exclusions_canonicalized_and_deduplicated() {
    let config = parse(&all("[STATUS.example.com, status.example.com]")).unwrap();

    assert_eq!(config.protection, Policy::All { excluded: [host("status.example.com")].into() });
}

#[test]
fn all_mode_without_exclusions_gates_every_host() {
    let config = parse("profile: test\nprotection:\n  mode: all\n").unwrap();

    assert_eq!(config.protection, Policy::All { excluded: [].into() });
    assert!(config.protection.is_gated(&host("anything.example.com")));
}

#[test]
fn hosts_are_rejected_in_all_mode() {
    assert!(parse("profile: test\nprotection:\n  mode: all\n  hosts: [a.example.com]\n").is_err());
    assert!(parse("profile: test\nprotection:\n  mode: all\n  hosts: []\n").is_err());
}

#[test]
fn excluded_hosts_are_rejected_in_selected_mode() {
    assert!(
        parse("profile: test\nprotection:\n  mode: selected\n  excluded_hosts: [a.example.com]\n").is_err()
    );
    assert!(parse("profile: test\nprotection:\n  mode: selected\n  excluded_hosts: []\n").is_err());
}

#[test]
fn wildcards_are_rejected() {
    assert!(parse(&selected("['*.example.com']")).is_err());
    assert!(parse(&all("['*.example.com']")).is_err());
}

#[test]
fn paths_ports_and_schemes_are_rejected() {
    assert!(parse(&selected("[example.com/app]")).is_err());
    assert!(parse(&selected("['https://example.com']")).is_err());
    assert!(parse(&all("['example.com:8443']")).is_err());
}

#[test]
fn the_portal_host_may_not_be_listed_in_either_mode() {
    let portal = "portal_host: Access.Example.com\n";

    assert!(parse(&format!("{portal}{}", selected("[access.example.com]"))).is_err());
    assert!(parse(&format!("{portal}{}", all("[access.example.com]"))).is_err());
}

#[test]
fn other_hosts_may_be_listed_next_to_a_portal_host() {
    let yaml = format!("portal_host: access.example.com\n{}", selected("[jellyfin.example.com]"));

    assert!(parse(&yaml).is_ok());
}

#[test]
fn an_empty_selection_loads_and_is_flagged() {
    let config = parse(&selected("[]")).unwrap();

    assert!(config.protection.gates_nothing());
    assert!(parse("profile: test\nprotection:\n  mode: selected\n").unwrap().protection.gates_nothing());
}

#[test]
fn a_non_empty_selection_is_not_flagged() {
    assert!(!parse(&selected("[a.example.com]")).unwrap().protection.gates_nothing());
}
