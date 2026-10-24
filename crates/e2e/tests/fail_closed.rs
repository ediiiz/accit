//! Failure and change: accit gone, a conflicting router, a Policy reload (docs/spec.md sections 4, 10, 14).

use std::time::Duration;

use anyhow::Result;
use e2e::stack::eventually;
use e2e::{ADDRESS_A, ADDRESS_B, GATED, OPEN, PORTAL, Session};

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn with_accit_stopped_every_host_is_denied() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;

    assert!(stack.unlock(ADDRESS_A)?.served());
    let control = stack.call(ADDRESS_A, GATED, "/").send()?;
    stack.stop_accit()?;
    let gated = stack.call(ADDRESS_A, GATED, "/").send()?;
    let open = stack.call(ADDRESS_B, OPEN, "/").send()?;
    let portal = stack.call(ADDRESS_A, PORTAL, "/api/status").with_certificate().send()?;

    assert!(control.reached_app(), "control: {control:?}");
    assert!(!gated.served(), "{gated:?}");
    assert!(!open.served(), "{open:?}");
    assert!(!portal.served(), "{portal:?}");

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn a_conflicting_portal_router_is_refused() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;

    stack.add_conflicting_router()?;
    // Strict mode renames both routers of the conflict and disables them, so the Portal host stops
    // answering instead of silently falling back to TLS options without client authentication.
    let name = "websecure-conflicted-conflict@file";
    let refused = eventually(Duration::from_secs(20), || Ok(stack.router(name)?.is_some()))?;
    let state = stack.router(name)?.unwrap_or_default();
    let anonymous = stack.call(ADDRESS_A, PORTAL, "/api/status").send()?;
    let credentialed = stack.call(ADDRESS_A, PORTAL, "/api/status").with_certificate().send()?;
    let fronted = stack.call(ADDRESS_A, OPEN, "/api/status").header("Host", PORTAL).send()?;

    assert!(refused, "Traefik never reported the conflicting router");
    assert!(state.contains("\"status\":\"disabled\""), "{state}");
    for reply in [anonymous, credentialed, fronted] {
        assert!(!reply.served(), "{reply:?}");
        assert!(!reply.body().contains("csrf_token"), "{reply:?}");
    }

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn switching_the_mode_applies_on_reload_and_keeps_grants() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;
    let selected = stack.base_config()?;
    let all = selected.replace("mode: selected\n  hosts:\n    - gated.accit.test\n", "mode: all\n");

    assert_ne!(all, selected, "the mode block was not found in harness/accit.yaml");
    assert!(stack.unlock(ADDRESS_A)?.served());
    let open_before = stack.call(ADDRESS_B, OPEN, "/").send()?;

    stack.set_config(&all)?;
    let open_unchanged = stack.call(ADDRESS_B, OPEN, "/").send()?;
    stack.reload()?;
    let open_after = stack.call(ADDRESS_B, OPEN, "/").send()?;
    let granted_after = stack.call(ADDRESS_A, GATED, "/").send()?;

    assert!(open_before.reached_app(), "{open_before:?}");
    assert!(open_unchanged.reached_app(), "a changed file alone changes nothing: {open_unchanged:?}");
    assert_eq!(open_after.status(), Some(403), "{open_after:?}");
    assert!(granted_after.reached_app(), "{granted_after:?}");

    Ok(())
}
