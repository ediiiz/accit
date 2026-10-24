//! The Portal behind mTLS (docs/spec.md sections 1 and 10).

use anyhow::Result;
use e2e::{ADDRESS_A, GATED, OPEN, PORTAL, Session};

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn portal_without_a_client_certificate_is_rejected() -> Result<()> {
    let session = Session::begin()?;

    let anonymous = session.stack.call(ADDRESS_A, PORTAL, "/api/status").send()?;
    let credentialed = session.stack.call(ADDRESS_A, PORTAL, "/api/status").with_certificate().send()?;

    assert!(!anonymous.served(), "{anonymous:?}");
    assert!(!anonymous.body().contains("csrf_token"), "{anonymous:?}");
    assert!(credentialed.served(), "control: {credentialed:?}");

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn portal_host_behind_another_host_name_is_refused() -> Result<()> {
    let session = Session::begin()?;

    // TLS options follow the SNI, routing follows the Host header: the Portal must not be reachable
    // by asking for it under a name that needs no client certificate.
    let fronted = session.stack.call(ADDRESS_A, OPEN, "/api/status").header("Host", PORTAL).send()?;
    let fronted_gated = session.stack.call(ADDRESS_A, OPEN, "/").header("Host", GATED).send()?;

    assert!(!fronted.served(), "{fronted:?}");
    assert!(!fronted.body().contains("csrf_token"), "{fronted:?}");
    assert!(!fronted_gated.reached_app(), "{fronted_gated:?}");

    Ok(())
}
