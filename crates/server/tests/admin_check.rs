//! `accit admin check [file]` (docs/spec.md section 11): validation without applying, the dry-run
//! coverage table, warnings and the client CA expiry.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

use std::path::Path;
use std::time::{Duration, SystemTime};

use server::check;

const NOW: SystemTime = SystemTime::UNIX_EPOCH;

fn check_config(yaml: &str) -> anyhow::Result<check::Report> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.yaml");
    std::fs::write(&path, yaml).unwrap();

    check::run(&path, NOW)
}

fn row<'a>(report: &'a check::Report, host: &str) -> Vec<&'a str> {
    report
        .text
        .lines()
        .find(|line| line.split_whitespace().next() == Some(host))
        .unwrap()
        .split_whitespace()
        .collect()
}

#[test]
fn the_coverage_table_marks_each_host_of_the_policy_gated_or_open() {
    let report = check_config(
        "profile: test
public_origin: https://portal.example.com
apps:
  - {name: Jellyfin, url: 'https://jellyfin.example.com/web'}
  - {name: Status, url: 'https://status.example.com'}
protection:
  mode: all
  excluded_hosts: [status.example.com]
",
    )
    .unwrap();

    assert_eq!(row(&report, "jellyfin.example.com"), ["jellyfin.example.com", "gated"]);
    assert_eq!(row(&report, "status.example.com"), ["status.example.com", "open"]);
    assert_eq!(row(&report, "portal.example.com"), ["portal.example.com", "open"]);
    assert!(!report.failed);
}

#[test]
fn all_mode_adds_a_row_for_every_other_host() {
    let report = check_config("profile: test\nprotection:\n  mode: all\n").unwrap();

    assert!(report.text.lines().any(|line| line == "other hosts  gated"), "{}", report.text);
}

#[test]
fn selected_mode_has_no_row_for_other_hosts() {
    let report =
        check_config("profile: test\nprotection:\n  mode: selected\n  hosts: [jellyfin.example.com]\n")
            .unwrap();

    assert!(!report.text.contains("other hosts"), "{}", report.text);
}

#[test]
fn selected_mode_lists_its_hosts_as_gated_and_other_app_hosts_as_open() {
    let report = check_config(
        "profile: test
apps:
  - {name: Wiki, url: 'https://wiki.example.com'}
protection:
  mode: selected
  hosts: [jellyfin.example.com]
",
    )
    .unwrap();

    assert_eq!(row(&report, "jellyfin.example.com"), ["jellyfin.example.com", "gated"]);
    assert_eq!(row(&report, "wiki.example.com"), ["wiki.example.com", "open"]);
}

#[test]
fn an_empty_selection_warns_that_no_host_requires_a_grant() {
    let report = check_config("profile: test\nprotection:\n  mode: selected\n").unwrap();

    assert!(
        report.text.contains("warning: protection: selected mode without hosts; no host requires a Grant")
    );
    assert!(!report.failed, "a warning is not a failure");
}

#[test]
fn a_selection_with_hosts_does_not_warn_about_an_empty_selection() {
    let report =
        check_config("profile: test\nprotection:\n  mode: selected\n  hosts: [jellyfin.example.com]\n")
            .unwrap();

    assert!(!report.text.contains("no host requires a Grant"), "{}", report.text);
}

#[test]
fn an_app_link_to_an_open_host_warns_and_one_to_a_gated_host_does_not() {
    let report = check_config(
        "profile: test
apps:
  - {name: Wiki, url: 'https://wiki.example.com'}
  - {name: Jellyfin, url: 'https://jellyfin.example.com'}
protection:
  mode: selected
  hosts: [jellyfin.example.com]
",
    )
    .unwrap();

    assert!(
        report.text.contains("warning: app link `Wiki` points to open host wiki.example.com"),
        "{}",
        report.text
    );
    assert!(!report.text.contains("app link `Jellyfin`"), "{}", report.text);
}

#[test]
fn an_invalid_file_is_an_error_naming_the_problem() {
    let error = check_config("profile: test\nprotection:\n  mode: everything\n").unwrap_err();

    assert!(format!("{error:#}").contains("everything"), "{error:#}");
}

#[test]
fn a_missing_file_is_an_error() {
    assert!(check::run(Path::new("/nonexistent/accit.yaml"), NOW).is_err());
}

const DAY: u64 = 86_400;
/// `notAfter` of the fixtures, from `openssl x509 -enddate` at generation.
const CA_A_EXPIRES: u64 = 4_946_474_234;
const CA_B_EXPIRES: u64 = 3_369_674_234;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn check_ca(ca_file: &str, now: u64) -> check::Report {
    let yaml = format!("profile: test\nclient_ca_file: {ca_file}\nprotection:\n  mode: all\n");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.yaml");
    std::fs::write(&path, yaml).unwrap();

    check::run(&path, SystemTime::UNIX_EPOCH + Duration::from_secs(now)).unwrap()
}

#[test]
fn the_client_ca_reports_its_days_to_expiry() {
    let report = check_ca(&fixture("client-ca-a.pem"), CA_A_EXPIRES - 31 * DAY);

    assert!(report.text.contains("client CA: 31 days to expiry"), "{}", report.text);
    assert!(!report.text.contains("warning: client CA"), "{}", report.text);
    assert!(!report.failed);
}

#[test]
fn the_client_ca_warns_from_30_days_before_expiry() {
    let report = check_ca(&fixture("client-ca-a.pem"), CA_A_EXPIRES - 30 * DAY);

    assert!(report.text.contains("warning: client CA: 30 days to expiry"), "{}", report.text);
    assert!(!report.failed, "a warning is not a failure");
}

#[test]
fn partial_days_round_down() {
    let report = check_ca(&fixture("client-ca-a.pem"), CA_A_EXPIRES - 5 * DAY - 3 * 3600);

    assert!(report.text.contains("warning: client CA: 5 days to expiry"), "{}", report.text);
}

#[test]
fn an_expired_client_ca_is_an_error() {
    let report = check_ca(&fixture("client-ca-a.pem"), CA_A_EXPIRES + 1);

    assert!(report.text.contains("error: client CA expired"), "{}", report.text);
    assert!(report.failed);
}

#[test]
fn an_unreadable_client_ca_is_an_error() {
    let report = check_ca("/nonexistent/ca.pem", 0);

    assert!(report.text.contains("error: client CA /nonexistent/ca.pem"), "{}", report.text);
    assert!(report.failed);
}

#[test]
fn a_file_without_a_certificate_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, "not a certificate\n").unwrap();

    let report = check_ca(&ca.display().to_string(), 0);

    assert!(report.text.contains("error: client CA"), "{}", report.text);
    assert!(report.failed);
}

#[test]
fn a_bundle_expires_with_its_earliest_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("bundle.pem");
    let both = [fixture("client-ca-a.pem"), fixture("client-ca-b.pem")]
        .map(|file| std::fs::read_to_string(file).unwrap())
        .concat();
    std::fs::write(&bundle, both).unwrap();

    let report = check_ca(&bundle.display().to_string(), CA_B_EXPIRES - 10 * DAY);

    assert!(report.text.contains("warning: client CA: 10 days to expiry"), "{}", report.text);
}

#[test]
fn without_client_ca_file_the_report_says_nothing_about_it() {
    let report = check_config("profile: test\nprotection:\n  mode: all\n").unwrap();

    assert!(!report.text.contains("client CA"), "{}", report.text);
}
