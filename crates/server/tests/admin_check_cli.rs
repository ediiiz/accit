//! `accit admin check [file]` through the binary: no daemon needed, the exit code carries the result.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

use std::path::Path;
use std::process::{Command, Output};

const CONFIG: &str = "profile: test
apps:
  - {name: Wiki, url: 'https://wiki.example.com'}
protection:
  mode: selected
  hosts: [jellyfin.example.com]
";

fn accit(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_accit"));
    command.args(args).env_remove("ACCIT_CONFIG");
    command
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn write(dir: &Path, yaml: &str) -> String {
    let path = dir.join("candidate.yaml");
    std::fs::write(&path, yaml).unwrap();

    path.display().to_string()
}

fn check(file: &str) -> Output {
    accit(&["admin", "check", file]).output().unwrap()
}

#[test]
fn help_lists_admin_check() {
    let output = accit(&["--help"]).output().unwrap();
    let help = format!("{}{}", text(&output.stdout), text(&output.stderr));

    assert!(output.status.success());
    assert!(help.contains("admin check [file]"), "{help}");
}

#[test]
fn a_valid_file_prints_the_coverage_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();

    let output = check(&write(dir.path(), CONFIG));

    assert!(output.status.success(), "{}", text(&output.stderr));
    let report = text(&output.stdout);
    assert!(report.contains("jellyfin.example.com  gated"), "{report}");
    assert!(report.contains("wiki.example.com  open"), "{report}");
    assert!(report.contains("warning: app link `Wiki` points to open host wiki.example.com"), "{report}");
}

#[test]
fn an_invalid_file_exits_non_zero_with_the_validation_error() {
    let dir = tempfile::tempdir().unwrap();

    let output = check(&write(dir.path(), "profile: test\nprotection:\n  mode: everything\n"));

    assert!(!output.status.success());
    assert!(text(&output.stderr).contains("everything"), "{}", text(&output.stderr));
}

#[test]
fn without_a_file_it_checks_the_running_config_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), CONFIG);

    let output = accit(&["admin", "check"]).env("ACCIT_CONFIG", &file).output().unwrap();

    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("jellyfin.example.com  gated"), "{}", text(&output.stdout));
}

#[test]
fn a_failed_check_prints_the_report_and_exits_non_zero() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = format!("{CONFIG}client_ca_file: {}/missing.pem\n", dir.path().display());

    let output = check(&write(dir.path(), &yaml));

    assert!(!output.status.success());
    assert!(text(&output.stdout).contains("error: client CA"), "{}", text(&output.stdout));
    assert!(text(&output.stdout).contains("jellyfin.example.com  gated"), "{}", text(&output.stdout));
}

#[test]
fn more_than_one_file_is_refused() {
    let output = accit(&["admin", "check", "a.yaml", "b.yaml"]).output().unwrap();

    assert!(!output.status.success());
}
