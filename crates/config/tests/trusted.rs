//! Trusted networks in the configuration (docs/spec.md sections 4, 5 and 12).

mod common;

use access::Address;
use common::parse;

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn protection(trusted: &str) -> String {
    format!("profile: test\nprotection:\n  mode: selected\n  hosts: [jellyfin.example.com]\n{trusted}")
}

const HOME: &str = "  trusted_networks:
    - 192.168.1.0/24
    - fd12::/16
  trusted_networks_exclude:
    - 192.168.1.1
";

#[test]
fn trusted_networks_and_exclusions_parse_inside_protection() {
    let config = parse(&protection(HOME)).unwrap();
    let trusted = &config.trusted_networks;

    assert!(trusted.contains(address("192.168.1.20")));
    assert!(trusted.contains(address("fd12::20")));
    assert!(!trusted.contains(address("192.168.1.1")));
    assert!(!trusted.contains(address("192.168.2.20")));
}

#[test]
fn trusted_networks_work_in_all_mode_and_default_to_none() {
    let all = "profile: test\nprotection:\n  mode: all\n  trusted_networks: [10.0.0.0/8]\n";
    let none = parse("profile: test\nprotection:\n  mode: all\n").unwrap();

    assert!(parse(all).unwrap().trusted_networks.contains(address("10.1.2.3")));
    assert!(!none.trusted_networks.contains(address("10.1.2.3")));
}

#[test]
fn an_empty_or_null_list_trusts_nothing() {
    for lists in ["  trusted_networks: []\n", "  trusted_networks:\n  trusted_networks_exclude:\n"] {
        let config = parse(&protection(lists)).unwrap();

        assert!(!config.trusted_networks.contains(address("192.168.1.20")), "{lists}");
    }
}

#[test]
fn public_ranges_and_everything_are_rejected() {
    for range in ["8.8.8.0/24", "0.0.0.0/0", "::/0", "2a01:4f8::/32", "172.0.0.0/8"] {
        let error = parse(&protection(&format!("  trusted_networks: [{range}]\n"))).unwrap_err();

        assert!(format!("{error:#}").contains(range), "{range}: {error:#}");
    }
}

#[test]
fn a_malformed_range_or_exclusion_is_rejected() {
    for lists in [
        "  trusted_networks: [192.168.1.1]\n",
        "  trusted_networks: [home]\n",
        "  trusted_networks: [192.168.1.0/24]\n  trusted_networks_exclude: [192.168.1.0/30]\n",
        "  trusted_networks: [192.168.1.0/24]\n  trusted_networks_exclude: [router]\n",
        "  trusted_networks: 192.168.1.0/24\n",
    ] {
        assert!(parse(&protection(lists)).is_err(), "{lists}");
    }
}

#[test]
fn an_exclusion_is_canonicalized_like_an_address() {
    let lists = "  trusted_networks: [192.168.1.0/24]\n  trusted_networks_exclude: ['::ffff:192.168.1.1']\n";
    let config = parse(&protection(lists)).unwrap();

    assert!(!config.trusted_networks.contains(address("192.168.1.1")));
}

#[test]
fn a_range_overlapping_a_docker_network_is_rejected() {
    let docker = "docker_networks: [172.30.0.0/24, 172.18.0.1/16]\n";

    for range in ["172.30.0.0/24", "172.16.0.0/12", "172.18.5.0/24", "172.30.0.128/25"] {
        let yaml = format!("{}{docker}", protection(&format!("  trusted_networks: [{range}]\n")));
        let error = parse(&yaml).unwrap_err();

        assert!(format!("{error:#}").contains(range), "{range}: {error:#}");
    }
}

#[test]
fn docker_networks_beside_the_trusted_ranges_are_accepted() {
    let yaml = format!("{}docker_networks: [172.30.0.0/24, 172.18.0.0/16]\n", protection(HOME));

    assert!(parse(&yaml).is_ok());
}

#[test]
fn docker_networks_must_be_ranges() {
    for value in ["172.30.0.0", "accit", "[172.30.0.0/24, nope]", "172.30.0.0/24"] {
        let yaml = format!("{}docker_networks: {value}\n", protection(HOME));

        assert!(parse(&yaml).is_err(), "{value}");
    }
}
