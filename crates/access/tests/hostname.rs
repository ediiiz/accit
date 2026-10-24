//! Hostname canonicalization for `X-Forwarded-Host` and for Policy entries (docs/spec.md sections 3 and 4).

use access::Hostname;

#[test]
fn hostnames_are_lowercased() {
    assert_eq!(Hostname::parse("Jellyfin.Example.COM").unwrap().as_str(), "jellyfin.example.com");
}

#[test]
fn one_trailing_dot_is_removed() {
    assert_eq!(Hostname::parse("photos.example.com.").unwrap().as_str(), "photos.example.com");
    assert!(Hostname::parse("photos.example.com..").is_err());
}

#[test]
fn unicode_names_become_their_ascii_form() {
    assert_eq!(Hostname::parse("bücher.example").unwrap().as_str(), "xn--bcher-kva.example");
}

#[test]
fn a_forwarded_host_may_carry_a_port() {
    let host = Hostname::from_forwarded_host("Access.Example.com:443".as_bytes()).unwrap();

    assert_eq!(host.as_str(), "access.example.com");
    assert_eq!(Hostname::from_forwarded_host(b"access.example.com").unwrap().as_str(), "access.example.com");
}

#[test]
fn policy_hostnames_may_not_carry_a_port() {
    assert!(Hostname::parse("access.example.com:443").is_err());
}

#[test]
fn malformed_forwarded_hosts_are_rejected() {
    let rejected: &[&[u8]] = &[
        b"",
        b"access.example.com:",
        b"access.example.com:https",
        b"access.example.com:443:443",
        b"access.example.com:99999",
        b"[2001:db8::1]:443",
        b"\xff.example.com",
    ];

    for value in rejected {
        assert!(Hostname::from_forwarded_host(value).is_err(), "{value:?} was accepted");
    }
}

#[test]
fn anything_but_a_plain_hostname_is_rejected() {
    let rejected = [
        "",
        ".",
        " access.example.com",
        "access.example.com ",
        "access .example.com",
        "user@access.example.com",
        "access.example.com/path",
        "access.example.com?x",
        "access.example.com#x",
        "access.example.com\\x",
        "*.example.com",
        "access.example.com,other.example.com",
        "access..example.com",
        "-access.example.com",
        "access-.example.com",
        "under_score.example.com",
        "203.0.113.7",
        "0x7f000001",
        "0X7F000001",
        "127.0.0.0x1",
        "example.0x10",
        "example.0x",
        "2001:db8::1",
        "[2001:db8::1]",
        "access\texample.com",
    ];

    for input in rejected {
        assert!(Hostname::parse(input).is_err(), "{input:?} was accepted");
    }
}

#[test]
fn labels_and_names_have_length_limits() {
    let long_label = "a".repeat(64);
    let long_name = format!("{}.example.com", vec!["a".repeat(63); 4].join("."));

    assert!(Hostname::parse(&format!("{}.example.com", "a".repeat(63))).is_ok());
    assert!(Hostname::parse(&format!("{long_label}.example.com")).is_err());
    assert!(Hostname::parse(&long_name).is_err());
}

#[test]
fn equal_hosts_compare_equal_after_canonicalization() {
    assert_eq!(
        Hostname::parse("Photos.Example.com.").unwrap(),
        Hostname::parse("photos.example.com").unwrap()
    );
}
