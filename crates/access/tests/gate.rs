//! The gate decision, steps 3 to 8 of docs/spec.md section 3. Steps 1 (readiness) and 2 (proxy
//! trust) belong to the verification listener.

use access::{Address, Cidr, Gate, Hostname, Policy, TrustedNetworks, Verdict};

const PORTAL: &str = "access.example.com";
const PUBLIC_V4: &str = "81.2.69.160";

#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn host(name: &str) -> Hostname {
    Hostname::parse(name).unwrap()
}

fn selected(hosts: &[&str]) -> Policy {
    Policy::Selected { hosts: hosts.iter().map(|name| host(name)).collect() }
}

/// One `X-Forwarded-For` and one `X-Forwarded-Host` value; production profile; no Grants.
fn decide(policy: &Policy, address: &str, host: &str) -> Verdict {
    decide_with(policy, &[address], &[host], |_| false)
}

fn decide_with(
    policy: &Policy,
    forwarded_for: &[&str],
    forwarded_host: &[&str],
    has_grant: impl FnOnce(Address) -> bool,
) -> Verdict {
    let portal = host(PORTAL);
    let gate =
        Gate { policy, trusted: &TrustedNetworks::default(), portal_host: Some(&portal), public_only: true };

    gate.decide(
        forwarded_for.iter().map(|value| value.as_bytes()),
        forwarded_host.iter().map(|value| value.as_bytes()),
        has_grant,
    )
}

#[test]
fn step_4_the_portal_host_passes_without_a_grant() {
    let policy = Policy::All { excluded: [].into() };

    assert_eq!(decide(&policy, PUBLIC_V4, PORTAL), Verdict::Pass);
}

#[test]
fn step_3_a_missing_repeated_or_malformed_forwarded_for_is_denied_even_on_the_portal() {
    let policy = selected(&[]);
    let invalid: [&[&str]; 6] = [
        &[],
        &[PUBLIC_V4, PUBLIC_V4],
        &["81.2.69.160, 81.2.69.161"],
        &["81.2.69.160:443"],
        &["not-an-address"],
        &[""],
    ];

    for forwarded_for in invalid {
        let verdict = decide_with(&policy, forwarded_for, &[PORTAL], |_| true);

        assert_eq!(verdict, Verdict::Deny, "{forwarded_for:?}");
    }
}

#[test]
fn step_7_a_gated_host_passes_only_for_the_address_with_a_grant() {
    let policy = selected(&["jellyfin.example.com"]);
    let granted = |address: Address| address.to_string() == PUBLIC_V4;

    let with = decide_with(&policy, &[PUBLIC_V4], &["jellyfin.example.com"], granted);
    let other = decide_with(&policy, &["81.2.69.161"], &["jellyfin.example.com"], granted);

    assert_eq!(with, Verdict::Pass);
    assert_eq!(other, Verdict::Deny);
}

#[test]
fn step_3_an_invalid_forwarded_host_is_denied_even_with_a_grant() {
    let policy = Policy::All { excluded: [].into() };
    let invalid: [&[&str]; 9] = [
        &[],
        &[""],
        &["jellyfin.example.com", "jellyfin.example.com"],
        &["jellyfin.example.com, evil.com"],
        &["user@jellyfin.example.com"],
        &["jellyfin.example.com/path"],
        &["*.example.com"],
        &["81.2.69.1"],
        &["[2a01:4f8::1]"],
    ];

    for forwarded_host in invalid {
        let verdict = decide_with(&policy, &[PUBLIC_V4], forwarded_host, |_| true);

        assert_eq!(verdict, Verdict::Deny, "{forwarded_host:?}");
    }
}

#[test]
fn step_6_selected_mode_opens_every_unlisted_host_including_newly_seen_ones() {
    let policy = selected(&["jellyfin.example.com"]);

    assert_eq!(decide(&policy, PUBLIC_V4, "photos.example.com"), Verdict::Pass);
    assert_eq!(decide(&policy, PUBLIC_V4, "brand-new.example.com"), Verdict::Pass);
    assert_eq!(decide(&policy, PUBLIC_V4, "jellyfin.example.com"), Verdict::Deny);
}

#[test]
fn step_6_all_mode_opens_only_excluded_hosts_and_gates_newly_seen_ones() {
    let policy = Policy::All { excluded: [host("status.example.com")].into() };

    assert_eq!(decide(&policy, PUBLIC_V4, "status.example.com"), Verdict::Pass);
    assert_eq!(decide(&policy, PUBLIC_V4, "brand-new.example.com"), Verdict::Deny);
    assert_eq!(decide(&policy, PUBLIC_V4, "jellyfin.example.com"), Verdict::Deny);
}

#[test]
fn hosts_are_compared_canonically() {
    let policy = selected(&["jellyfin.example.com"]);

    for spelling in ["JELLYFIN.example.com", "jellyfin.example.com.", "jellyfin.example.com:443"] {
        assert_eq!(decide(&policy, PUBLIC_V4, spelling), Verdict::Deny, "{spelling}");
    }

    for spelling in ["Access.Example.com", "access.example.com.", "access.example.com:443"] {
        assert_eq!(decide(&policy, PUBLIC_V4, spelling), Verdict::Pass, "{spelling}");
    }
}

const NON_PUBLIC: [&str; 6] =
    ["192.168.1.20", "10.0.0.5", "100.64.0.1", "127.0.0.1", "fd00::1", "2001:db8::1"];

#[test]
fn step_3_production_rejects_a_non_public_address_before_any_pass() {
    let policy = selected(&["jellyfin.example.com"]);

    for address in NON_PUBLIC {
        for host in [PORTAL, "photos.example.com", "jellyfin.example.com"] {
            let verdict = decide_with(&policy, &[address], &[host], |_| true);

            assert_eq!(verdict, Verdict::Deny, "{address} {host}");
        }
    }
}

#[test]
fn the_test_profile_allows_non_public_addresses() {
    let policy = selected(&["jellyfin.example.com"]);
    let portal = host(PORTAL);
    let trusted = TrustedNetworks::default();
    let gate = Gate { policy: &policy, trusted: &trusted, portal_host: Some(&portal), public_only: false };

    for address in NON_PUBLIC {
        let open = gate.decide([address.as_bytes()], [b"photos.example.com".as_slice()], |_| false);
        let gated = gate.decide([address.as_bytes()], [b"jellyfin.example.com".as_slice()], |_| true);

        assert_eq!(open, Verdict::Pass, "{address}");
        assert_eq!(gated, Verdict::Pass, "{address}");
    }
}

#[test]
fn public_ipv6_and_6to4_addresses_pass_in_production() {
    let policy = selected(&[]);

    for address in ["2a01:4f8::1", "2002:5102:45a0::1", "::ffff:81.2.69.160"] {
        assert_eq!(decide(&policy, address, "photos.example.com"), Verdict::Pass, "{address}");
    }
}

/// `192.168.1.0/24` and `fd12::/16` trusted, minus the router `192.168.1.1`.
#[allow(clippy::unwrap_used, reason = "test helper: an invalid fixture is a test bug")]
fn home() -> TrustedNetworks {
    let ranges = ["192.168.1.0/24", "fd12::/16"].map(|text| text.parse::<Cidr>().unwrap());

    TrustedNetworks::new(ranges, ["192.168.1.1".parse().unwrap()], &[]).unwrap()
}

fn decide_at_home(
    policy: &Policy,
    public_only: bool,
    address: &str,
    host: &str,
    has_grant: impl FnOnce(Address) -> bool,
) -> Verdict {
    let trusted = home();
    let gate = Gate { policy, trusted: &trusted, portal_host: None, public_only };

    gate.decide([address.as_bytes()], [host.as_bytes()], has_grant)
}

#[test]
fn step_5_a_trusted_address_passes_every_host_without_a_grant_in_both_modes() {
    let selected = selected(&["jellyfin.example.com"]);
    let all = Policy::All { excluded: [].into() };

    for policy in [&selected, &all] {
        for address in ["192.168.1.20", "fd12::20"] {
            let verdict = decide_at_home(policy, true, address, "jellyfin.example.com", |_| false);

            assert_eq!(verdict, Verdict::Pass, "{address}");
        }
    }
}

#[test]
fn step_5_an_excluded_address_needs_a_grant_and_a_production_non_public_one_is_rejected() {
    let policy = selected(&["jellyfin.example.com"]);

    let test_profile = decide_at_home(&policy, false, "192.168.1.1", "jellyfin.example.com", |_| false);
    let production = decide_at_home(&policy, true, "192.168.1.1", "photos.example.com", |_| true);

    assert_eq!(test_profile, Verdict::Deny);
    assert_eq!(production, Verdict::Deny);
}

#[test]
fn step_5_a_private_address_outside_every_trusted_network_still_needs_a_grant() {
    let policy = selected(&["jellyfin.example.com"]);

    let production = decide_at_home(&policy, true, "192.168.2.20", "photos.example.com", |_| true);
    let test_profile = decide_at_home(&policy, false, "192.168.2.20", "jellyfin.example.com", |_| false);

    assert_eq!(production, Verdict::Deny);
    assert_eq!(test_profile, Verdict::Deny);
}

#[test]
fn step_5_a_public_ipv6_source_inside_the_home_still_needs_a_grant() {
    let policy = Policy::All { excluded: [].into() };
    let address = "2a01:4f8::1";

    let without = decide_at_home(&policy, true, address, "jellyfin.example.com", |_| false);
    let with = decide_at_home(&policy, true, address, "jellyfin.example.com", |_| true);

    assert_eq!(without, Verdict::Deny);
    assert_eq!(with, Verdict::Pass);
}
