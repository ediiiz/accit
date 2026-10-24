//! The gate decision as a client sees it through Traefik (docs/spec.md sections 3 and 10).

use anyhow::Result;
use e2e::{ADDRESS_A, ADDRESS_B, GATED, OPEN, Session};

const TOKEN_HEADER: &str = "X-Accit-Proxy-Token";

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn unlock_allows_the_gated_host_from_that_address_only() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;

    let before = stack.call(ADDRESS_A, GATED, "/").send()?;
    let unlock = stack.unlock(ADDRESS_A)?;
    let from_a = stack.call(ADDRESS_A, GATED, "/").send()?;
    let from_b = stack.call(ADDRESS_B, GATED, "/").send()?;

    assert_eq!(before.status(), Some(403), "{before:?}");
    assert!(unlock.served(), "{unlock:?}");
    assert!(from_a.reached_app(), "{from_a:?}");
    assert_eq!(from_b.status(), Some(403), "{from_b:?}");
    assert_eq!(from_b.body(), "Forbidden");

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn open_host_passes_without_a_grant() -> Result<()> {
    let session = Session::begin()?;

    let open = session.stack.call(ADDRESS_B, OPEN, "/").send()?;

    assert!(open.reached_app(), "{open:?}");

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn spoofed_forwarding_headers_and_host_do_not_pass() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;

    assert!(stack.unlock(ADDRESS_A)?.served());
    let control = stack.call(ADDRESS_A, GATED, "/").send()?;
    let address = stack.call(ADDRESS_B, GATED, "/").header("X-Forwarded-For", ADDRESS_A).send()?;
    let host = stack.call(ADDRESS_B, GATED, "/").header("X-Forwarded-Host", e2e::PORTAL).send()?;
    let rewritten = stack.call(ADDRESS_B, OPEN, "/").header("Host", GATED).send()?;

    assert!(control.reached_app(), "control: {control:?}");
    assert_eq!(address.status(), Some(403), "{address:?}");
    assert_eq!(host.status(), Some(403), "{host:?}");
    assert_eq!(rewritten.status(), Some(403), "{rewritten:?}");

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn the_proxy_token_never_reaches_the_app() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;
    let token = stack.token()?;

    assert!(stack.unlock(ADDRESS_A)?.served());
    let gated = stack.call(ADDRESS_A, GATED, "/").send()?;
    let open = stack.call(ADDRESS_A, OPEN, "/").send()?;

    for reply in [gated, open] {
        let echoed = reply.body().to_lowercase();

        assert!(reply.reached_app(), "{reply:?}");
        assert!(echoed.contains("x-forwarded-for"), "control: whoami echoes request headers");
        assert!(!echoed.contains(&TOKEN_HEADER.to_lowercase()), "{reply:?}");
        assert!(!reply.body().contains(&token), "{reply:?}");
    }

    Ok(())
}

#[test]
#[ignore = "needs the Compose harness: scripts/e2e"]
fn a_missing_or_wrong_proxy_token_is_denied() -> Result<()> {
    let session = Session::begin()?;
    let stack = &session.stack;
    let token = stack.token()?;
    let from = ("X-Forwarded-For", ADDRESS_A);
    let host = ("X-Forwarded-Host", GATED);

    assert!(stack.unlock(ADDRESS_A)?.served());
    let valid = stack.verify_as_traefik(&[from, host, (TOKEN_HEADER, &token)])?;
    let missing = stack.verify_as_traefik(&[from, host])?;
    let wrong = stack.verify_as_traefik(&[from, host, (TOKEN_HEADER, "not-the-token")])?;

    assert_eq!(valid, Some(204), "control: the Granted Address passes with the token");
    assert_eq!(missing, Some(403));
    assert_eq!(wrong, Some(403));

    Ok(())
}
