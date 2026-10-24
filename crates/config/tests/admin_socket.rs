//! Where the admin socket lives (docs/spec.md sections 11 and 12): `admin_socket`, else the data
//! directory, the one holding `database`.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::path::Path;

use common::with;

#[test]
fn admin_socket_wins() {
    let config = with("database: /data/accit.db\nadmin_socket: /run/accit/admin.sock\n").unwrap();

    assert_eq!(config.admin_socket.as_deref(), Some(Path::new("/run/accit/admin.sock")));
}

#[test]
fn without_admin_socket_it_sits_in_the_data_directory() {
    let config = with("database: /data/accit.db\n").unwrap();

    assert_eq!(config.admin_socket.as_deref(), Some(Path::new("/data/admin.sock")));
}

#[test]
fn without_either_there_is_no_socket() {
    assert_eq!(with("").unwrap().admin_socket, None);
}

/// `accit admin reload` must reach the daemon to hear why a file is invalid.
#[test]
fn the_socket_is_found_in_a_file_that_does_not_validate() {
    let invalid = "profile: production\nprotection:\n  mode: everything\ndatabase: /data/accit.db\n";

    assert_eq!(config::admin_socket(invalid).unwrap().as_deref(), Some(Path::new("/data/admin.sock")));
    assert_eq!(
        config::admin_socket("admin_socket: /run/admin.sock\nunknown: 1\n").unwrap().as_deref(),
        Some(Path::new("/run/admin.sock"))
    );
    assert_eq!(config::admin_socket("profile: test\n").unwrap(), None);
}

#[test]
fn yaml_that_does_not_parse_is_an_error() {
    assert!(config::admin_socket("admin_socket: [").is_err());
}
