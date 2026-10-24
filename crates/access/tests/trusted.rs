//! Trusted networks (docs/spec.md section 5): which ranges are accepted, and which Addresses they cover.

use access::{Address, Cidr, TrustedError, TrustedNetworks};

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn cidr(text: &str) -> Cidr {
    text.parse().unwrap()
}

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn networks(trusted: &[&str], excluded: &[&str], docker: &[&str]) -> Result<TrustedNetworks, TrustedError> {
    let docker: Vec<Cidr> = docker.iter().map(|text| cidr(text)).collect();

    TrustedNetworks::new(
        trusted.iter().map(|text| cidr(text)),
        excluded.iter().map(|text| address(text)),
        &docker,
    )
}

#[test]
fn a_range_needs_a_prefix_length_within_its_family() {
    for text in
        ["192.168.1.0", "192.168.1.0/33", "fd00::/129", "192.168.1.0/", "192.168.1.0/x", "nope/8", "/8"]
    {
        assert!(text.parse::<Cidr>().is_err(), "{text}");
    }
}

#[test]
fn private_unique_local_and_cgnat_ranges_are_accepted() {
    for range in [
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "100.64.0.0/10",
        "fc00::/7",
        "192.168.1.0/24",
        "10.1.2.3/32",
        "fd12:3456::/48",
    ] {
        assert!(networks(&[range], &[], &[]).is_ok(), "{range}");
    }
}

#[test]
fn public_and_unspecified_ranges_are_rejected() {
    for range in [
        "0.0.0.0/0",
        "::/0",
        "8.8.8.0/24",
        "172.0.0.0/8",
        "172.16.0.0/11",
        "192.0.0.0/8",
        "100.0.0.0/8",
        "2606:4700::/32",
        "2000::/3",
        "fe80::/10",
        "fc00::/6",
        "::ffff:192.168.0.0/112",
        "127.0.0.0/8",
        "169.254.0.0/16",
    ] {
        assert!(matches!(networks(&[range], &[], &[]), Err(TrustedError::NotPrivate(_))), "{range}");
    }
}

#[test]
fn one_public_range_rejects_the_whole_list() {
    assert!(networks(&["192.168.1.0/24", "8.8.8.0/24"], &[], &[]).is_err());
}

#[test]
fn a_range_overlapping_a_docker_network_is_rejected() {
    for (range, docker) in [
        ("172.18.0.0/16", "172.18.0.0/16"),
        ("172.16.0.0/12", "172.18.0.0/16"),
        ("172.18.0.0/24", "172.18.0.0/16"),
        ("172.18.5.0/24", "172.18.0.1/16"),
        ("fd00::/8", "fd00:1::/64"),
    ] {
        assert!(
            matches!(networks(&[range], &[], &[docker]), Err(TrustedError::OverlapsDocker { .. })),
            "{range} {docker}"
        );
    }
}

#[test]
fn a_range_beside_a_docker_network_is_accepted() {
    assert!(networks(&["192.168.1.0/24", "fd12::/16"], &[], &["172.18.0.0/16", "fd00:1::/64"]).is_ok());
}

#[test]
fn a_trusted_address_is_inside_a_range_and_not_excluded() {
    let trusted = networks(&["192.168.1.0/24", "fd12::/16"], &["192.168.1.1"], &[]).unwrap_or_default();

    assert!(trusted.contains(address("192.168.1.20")));
    assert!(trusted.contains(address("fd12::20")));
    assert!(!trusted.contains(address("192.168.1.1")));
    assert!(!trusted.contains(address("192.168.2.20")));
    assert!(!trusted.contains(address("81.2.69.160")));
    assert!(!trusted.contains(address("2606:4700::1111")));
}

#[test]
fn an_ipv4_mapped_address_is_matched_as_ipv4() {
    let trusted = networks(&["192.168.1.0/24"], &["192.168.1.1"], &[]).unwrap_or_default();

    assert!(trusted.contains(address("::ffff:192.168.1.20")));
    assert!(!trusted.contains(address("::ffff:192.168.1.1")));
}

#[test]
fn without_ranges_nothing_is_trusted() {
    assert!(!TrustedNetworks::default().contains(address("192.168.1.20")));
}
