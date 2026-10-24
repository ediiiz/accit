//! The proxy token is read from `proxy_token_file` and never printed (docs/spec.md sections 10 and 13).

mod common;

use std::io;
use std::path::{Path, PathBuf};

use common::{SELECTED, parse_with_file, with};
use config::Config;

const SECRET: &str = "s3cr3t-proxy-token-value";

fn with_token_file() -> String {
    format!("{SELECTED}proxy_token_file: /secrets/proxy-token\n")
}

#[test]
fn the_token_is_read_from_the_configured_file() {
    let mut asked = None;
    let config = Config::parse(&with_token_file(), |path: &Path| {
        asked = Some(path.to_path_buf());
        Ok(SECRET.to_owned())
    })
    .unwrap();

    assert_eq!(asked, Some(PathBuf::from("/secrets/proxy-token")));
    assert_eq!(config.proxy_token.unwrap().expose(), SECRET);
}

#[test]
fn surrounding_whitespace_in_the_file_is_not_part_of_the_token() {
    let config = parse_with_file(&with_token_file(), "  s3cr3t-proxy-token-value \r\n").unwrap();

    assert_eq!(config.proxy_token.unwrap().expose(), SECRET);
}

#[test]
fn an_empty_token_file_is_rejected() {
    assert!(parse_with_file(&with_token_file(), "\n").is_err());
}

#[test]
fn an_unreadable_token_file_is_rejected() {
    let result = Config::parse(&with_token_file(), |_| Err(io::ErrorKind::PermissionDenied.into()));

    assert!(result.is_err());
}

#[test]
fn without_a_token_file_nothing_is_read() {
    let config = Config::parse(SELECTED, |_| panic!("no file was configured")).unwrap();

    assert!(config.proxy_token.is_none());
}

#[test]
fn the_token_never_appears_in_debug_output() {
    let config = parse_with_file(&with_token_file(), SECRET).unwrap();

    assert!(!format!("{config:?}").contains(SECRET));
    assert!(!format!("{config:#?}").contains(SECRET));
    assert!(!format!("{:?}", config.proxy_token.unwrap()).contains(SECRET));
}

#[test]
fn the_token_never_appears_in_errors() {
    let yaml = format!("{}max_grants: nope\n", with_token_file());
    let error = parse_with_file(&yaml, SECRET).unwrap_err();

    assert!(!format!("{error:?}").contains(SECRET));
    assert!(with("").is_ok());
}
