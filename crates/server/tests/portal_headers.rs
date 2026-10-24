//! Headers on every Portal response (docs/spec.md section 8): security, caching, no CORS.

mod common;

use std::borrow::Cow;

use axum::http::{Method, StatusCode};
use common::harness::{Harness, Reply, ready};
use common::{header, replacing, without};
use server::portal::Files;

fn assert_secure(reply: &Reply, what: &str) {
    let csp = reply.header("content-security-policy").unwrap_or_default();

    assert!(!csp.is_empty(), "{what}: no CSP");

    for directive in [
        "default-src 'self'",
        "connect-src 'self'",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.split("; ").any(|part| part == directive), "{what}: {directive} in {csp}");
    }

    assert!(!csp.contains("unsafe"), "{what}: {csp}");
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"), "{what}");
    assert_eq!(reply.header("referrer-policy"), Some("no-referrer"), "{what}");
    assert!(reply.header("strict-transport-security").is_none(), "{what}: HSTS is off by default");
}

#[tokio::test]
async fn every_portal_response_carries_the_security_headers() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let replies = [
        ("status", browser.request(Method::GET, "/api/status", vec![], "").await),
        ("unlock", browser.post("/api/unlock", form.clone(), "{}").await),
        ("refused unlock", browser.post("/api/unlock", without(form, "origin"), "{}").await),
        ("unknown api path", browser.request(Method::GET, "/api/other", vec![], "").await),
        ("wrong method", browser.request(Method::PUT, "/api/unlock", vec![], "").await),
        ("page", browser.request(Method::GET, "/", vec![], "").await),
        ("missing page", browser.request(Method::GET, "/missing", vec![], "").await),
    ];

    for (what, reply) in &replies {
        assert_secure(reply, what);
    }
}

#[tokio::test]
async fn the_gate_refusal_carries_them_too() {
    let harness = Harness::start().await;
    let reply = harness.without_token("/api/status").await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_secure(&reply, "gate refusal");
}

#[tokio::test]
async fn api_answers_are_never_stored() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let replies = [
        browser.request(Method::GET, "/api/status", vec![], "").await,
        browser.post("/api/unlock", form.clone(), "{}").await,
        browser.post("/api/revoke", form.clone(), "{}").await,
        browser.post("/api/unlock", without(form, "origin"), "{}").await,
        browser.request(Method::GET, "/api/other", vec![], "").await,
    ];

    for reply in &replies {
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{}", reply.status);
    }
}

#[tokio::test]
async fn there_is_no_cors() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let preflight = vec![
        header("origin", "https://evil.example.com"),
        header("access-control-request-method", "POST"),
        header("access-control-request-headers", "content-type"),
    ];
    let replies = [
        browser.request(Method::OPTIONS, "/api/unlock", preflight, "").await,
        browser
            .request(Method::GET, "/api/status", vec![header("origin", "https://evil.example.com")], "")
            .await,
        browser.post("/api/unlock", replacing(form, "origin", "https://evil.example.com"), "{}").await,
    ];

    for reply in &replies {
        assert!(reply.headers.keys().all(|name| !name.as_str().starts_with("access-control-")), "{reply:?}");
    }

    assert_eq!(replies[0].status, StatusCode::METHOD_NOT_ALLOWED);
}

fn site(files: &'static [(&str, &str)]) -> Files {
    Files::new(move |path| {
        files.iter().find(|(name, _)| *name == path).map(|(_, body)| Cow::Borrowed(body.as_bytes()))
    })
}

/// SHA-256 of `boot(1)`, from `printf 'boot(1)' | openssl dgst -sha256 -binary | base64`.
const BOOT_HASH: &str = "'sha256-2DybWYCpP5bHxOJSoSgyyMeFgTOXm5f6FasJ7cSODcA='";

#[tokio::test]
async fn a_page_allows_its_own_inline_script_and_nothing_else_does() {
    let harness = Harness::serving(site(&[
        ("index.html", "<html><body><script>boot(1)</script></body></html>"),
        ("app.js", "boot(1)"),
    ]))
    .await;
    let browser = harness.browser();
    let page = browser.request(Method::GET, "/", vec![], "").await;
    let asset = browser.request(Method::GET, "/app.js", vec![], "").await;
    let csp = page.header("content-security-policy").unwrap_or_default();

    assert_eq!(page.status, StatusCode::OK);
    assert!(csp.split("; ").any(|part| part == format!("script-src 'self' {BOOT_HASH}")), "{csp}");
    assert_eq!(asset.status, StatusCode::OK);
    assert!(!asset.header("content-security-policy").unwrap_or_default().contains("script-src"));
}
