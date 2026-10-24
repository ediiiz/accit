//! The production example in the operator guide (docs/guides/operator.md) must load, so the guide
//! cannot drift from the schema.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::path::Path;

use common::{host, parse_with_file};

/// The guide's fenced YAML block that starts with `profile: production`.
fn example() -> String {
    let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/guides/operator.md");
    let text = std::fs::read_to_string(guide).unwrap();
    let start = text.find("```yaml\nprofile: production").unwrap() + "```yaml\n".len();
    let length = text[start..].find("```").unwrap();

    text[start..start + length].to_owned()
}

#[test]
fn the_guides_production_example_loads_and_gates_what_it_says() {
    let config = parse_with_file(&example(), "token").unwrap();

    assert_eq!(config.portal_host, Some(host("portal.example.com")));
    assert!(config.protection.listed().any(|listed| *listed == host("jellyfin.example.com")));
    assert_eq!(config.trusted_proxy.map(|address| address.to_string()).as_deref(), Some("172.29.20.2"));
    assert_eq!(config.apps.len(), 1);
}
