//! Address parsing from Traefik's `X-Forwarded-For` (docs/spec.md section 3, step 3).

use access::{Address, AddressError, Family};

fn parse(values: &[&str]) -> Result<Address, AddressError> {
    Address::from_forwarded_for(values.iter().map(|value| value.as_bytes()))
}

#[test]
fn a_single_ipv4_value_is_the_address() {
    let address = parse(&["203.0.113.7"]).unwrap();

    assert_eq!(address.to_string(), "203.0.113.7");
    assert_eq!(address.family(), Family::V4);
}

#[test]
fn ipv6_is_written_in_its_canonical_form() {
    let address = parse(&["2A00:1450:4001:0828:0000:0000:0000:200E"]).unwrap();

    assert_eq!(address.to_string(), "2a00:1450:4001:828::200e");
    assert_eq!(address.family(), Family::V6);
}

#[test]
fn ipv4_mapped_ipv6_becomes_ipv4() {
    let address = parse(&["::ffff:198.51.100.20"]).unwrap();

    assert_eq!(address.to_string(), "198.51.100.20");
    assert_eq!(address.family(), Family::V4);
}

#[test]
fn ambiguous_or_malformed_input_is_rejected() {
    let rejected: &[&[&str]] = &[
        &[],
        &[""],
        &["203.0.113.7, 198.51.100.1"],
        &["203.0.113.7,198.51.100.1"],
        &["203.0.113.7", "198.51.100.1"],
        &[" 203.0.113.7"],
        &["203.0.113.7 "],
        &["203.0.113.7:443"],
        &["[2001:db8::1]"],
        &["[2001:db8::1]:443"],
        &["fe80::1%eth0"],
        &["unknown"],
        &["010.0.0.1"],
        &["203.0.113"],
        &["2001:db8::1/64"],
        &["for=203.0.113.7"],
    ];

    for values in rejected {
        assert!(parse(values).is_err(), "{values:?} was accepted");
    }
}

#[test]
fn non_utf8_is_rejected() {
    let value: &[u8] = &[0xff, 0xfe];

    assert!(Address::from_forwarded_for([value]).is_err());
}

#[test]
fn public_addresses() {
    for public in ["8.8.8.8", "1.1.1.1", "185.199.108.153", "2a00:1450:4001:828::200e", "2606:4700::1111"] {
        assert!(parse(&[public]).unwrap().is_public(), "{public} should be public");
    }
}

#[test]
fn non_public_addresses() {
    let non_public = [
        "0.0.0.0",
        "10.1.2.3",
        "100.64.0.1",
        "100.127.255.254",
        "127.0.0.1",
        "169.254.1.1",
        "172.16.0.1",
        "172.31.255.255",
        "192.0.0.8",
        "192.0.2.1",
        "192.168.1.50",
        "198.18.0.1",
        "198.51.100.20",
        "203.0.113.7",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
        "3fff::1",
        "64:ff9b::808:808",
        "100::1",
    ];

    for address in non_public {
        assert!(!parse(&[address]).unwrap().is_public(), "{address} should not be public");
    }
}

#[test]
fn public_ipv4_stays_public_after_unmapping() {
    assert!(parse(&["::ffff:8.8.8.8"]).unwrap().is_public());
    assert!(!parse(&["::ffff:192.168.1.1"]).unwrap().is_public());
}
