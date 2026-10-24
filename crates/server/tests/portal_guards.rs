//! What a mutation must prove, and what every Portal response carries (docs/spec.md section 8).

mod common;

use axum::http::{Method, StatusCode};
use common::harness::{Harness, ORIGIN, address, ready};
use common::{header, replacing, without};

const CLIENT: &str = "93.184.216.34";

fn granted(harness: &Harness) -> bool {
    harness.grants.active_until(address(CLIENT), harness.now()).is_some()
}

#[tokio::test]
async fn a_cross_origin_post_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    for origin in [
        "https://evil.example.com",
        "http://access.example.com",
        "https://access.example.com:8443",
        "https://access.example.com.evil.com",
        "https://sub.access.example.com",
        "https://ACCESS.example.com",
        "https://access.example.com/",
        "null",
        "",
    ] {
        let reply = browser.post("/api/unlock", replacing(form.clone(), "origin", origin), "{}").await;

        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{origin}");
    }

    assert!(!granted(&harness));
}

#[tokio::test]
async fn a_post_without_origin_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    let reply = browser.post("/api/unlock", without(form, "origin"), "{}").await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert!(!granted(&harness));
}

#[tokio::test]
async fn two_origin_headers_are_refused() {
    let harness = Harness::start().await;
    let (browser, mut form) = ready(&harness).await;
    form.push(header("origin", "https://evil.example.com"));

    assert_eq!(browser.post("/api/unlock", form, "{}").await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn without_a_public_origin_no_post_passes() {
    let harness = Harness::with(
        "profile: test
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
protection:
  mode: selected
",
    )
    .await;
    let (browser, form) = ready(&harness).await;

    assert_eq!(browser.post("/api/unlock", form, "{}").await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_default_port_in_public_origin_matches_the_origin_without_it() {
    let harness =
        Harness::replacing("https://access.example.com\n", "https://access.example.com:443\n").await;
    let (browser, form) = ready(&harness).await;

    let with_port = browser.post(
        "/api/unlock",
        replacing(form.clone(), "origin", "https://access.example.com:443"),
        "{}",
    );
    let without_port = browser.post("/api/unlock", replacing(form, "origin", ORIGIN), "{}");

    assert_eq!(with_port.await.status, StatusCode::FORBIDDEN);
    assert_eq!(without_port.await.status, StatusCode::OK);
}

#[tokio::test]
async fn another_port_in_public_origin_must_be_in_the_origin() {
    let harness =
        Harness::replacing("https://access.example.com\n", "https://access.example.com:8443\n").await;
    let (browser, form) = ready(&harness).await;

    let bare = browser.post("/api/unlock", replacing(form.clone(), "origin", ORIGIN), "{}").await;
    let exact =
        browser.post("/api/unlock", replacing(form, "origin", "https://access.example.com:8443"), "{}").await;

    assert_eq!(bare.status, StatusCode::FORBIDDEN);
    assert_eq!(exact.status, StatusCode::OK);
}

#[tokio::test]
async fn only_json_is_accepted() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    for content_type in
        ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data", "application/jsonp", ""]
    {
        let reply =
            browser.post("/api/unlock", replacing(form.clone(), "content-type", content_type), "{}").await;

        assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{content_type}");
    }

    let reply = browser.post("/api/unlock", without(form.clone(), "content-type"), "{}").await;

    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(!granted(&harness));

    let parameters = replacing(form, "content-type", "Application/JSON; charset=utf-8");

    assert_eq!(browser.post("/api/unlock", parameters, "{}").await.status, StatusCode::OK);
}

#[tokio::test]
async fn a_bad_csrf_token_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let token = browser.csrf().to_owned();
    let other = harness.browser().status().await.json()["csrf_token"].as_str().unwrap().to_owned();
    let flipped = if token.ends_with('0') { '1' } else { '0' };
    let same_length_wrong = format!("{}{flipped}", &token[..token.len() - 1]);

    for sent in [String::new(), "x".to_owned(), same_length_wrong, format!("{token}0"), other] {
        let reply = browser.post("/api/unlock", replacing(form.clone(), "x-csrf-token", &sent), "{}").await;

        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{sent}");
    }

    let missing = browser.post("/api/unlock", without(form.clone(), "x-csrf-token"), "{}").await;
    let mut twice = form;
    twice.push(header("x-csrf-token", &token));

    assert_eq!(missing.status, StatusCode::FORBIDDEN);
    assert_eq!(browser.post("/api/unlock", twice, "{}").await.status, StatusCode::FORBIDDEN);
    assert!(!granted(&harness));
}

#[tokio::test]
async fn the_token_is_bound_to_its_session() {
    let harness = Harness::start().await;
    let (_, form) = ready(&harness).await;
    let mut stranger = harness.browser();
    stranger.status().await;

    let borrowed = replacing(
        stranger.form(),
        "x-csrf-token",
        form.iter().find(|(name, _)| name == "x-csrf-token").unwrap().1.as_str(),
    );

    assert_eq!(stranger.post("/api/unlock", borrowed, "{}").await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_post_without_a_session_cookie_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    for cookie in [None, Some("__Host-accit=unknown"), Some("other=1"), Some("__Host-accit=")] {
        let headers = cookie.map_or_else(
            || without(form.clone(), "cookie"),
            |cookie| replacing(form.clone(), "cookie", cookie),
        );

        assert_eq!(
            browser.post("/api/unlock", headers, "{}").await.status,
            StatusCode::FORBIDDEN,
            "{cookie:?}"
        );
    }
}

#[tokio::test]
async fn the_cookie_may_sit_among_others() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let cookie = browser.cookie().to_owned();
    let headers = replacing(form, "cookie", &format!("a=1; {cookie}; b=2"));

    assert_eq!(browser.post("/api/unlock", headers, "{}").await.status, StatusCode::OK);
}

#[tokio::test]
async fn an_expired_session_cannot_mutate() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    harness.advance(std::time::Duration::from_mins(15));

    assert_eq!(browser.post("/api/unlock", form, "{}").await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_oversized_body_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;
    let padding = " ".repeat(2048);

    let reply = browser.post("/api/unlock", form, &format!("{{{padding}}}")).await;

    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!granted(&harness));
}

#[tokio::test]
async fn a_body_with_fields_or_not_an_object_is_refused() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    for body in
        [r#"{"extra":1}"#, r#"{"extra":null}"#, "[]", "null", "1", r#""{}""#, "", "{", "{} {}", "not json"]
    {
        let reply = browser.post("/api/unlock", form.clone(), body).await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
    }

    assert!(!granted(&harness));
    assert_eq!(browser.post("/api/unlock", form, " {} ").await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_endpoints_take_only_their_method() {
    let harness = Harness::start().await;
    let (browser, form) = ready(&harness).await;

    for (method, path) in [
        (Method::POST, "/api/status"),
        (Method::GET, "/api/unlock"),
        (Method::GET, "/api/revoke"),
        (Method::PUT, "/api/unlock"),
        (Method::DELETE, "/api/revoke"),
    ] {
        let reply = browser.request(method.clone(), path, form.clone(), "{}").await;

        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED, "{method} {path}");
    }

    assert!(!granted(&harness));
}

#[tokio::test]
async fn an_unknown_api_path_is_not_found() {
    let harness = Harness::start().await;
    let reply = harness.browser().request(Method::GET, "/api/other", vec![], "").await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_address_in_doubt_is_forbidden() {
    let harness = Harness::start().await;

    for address in [
        "not-an-ip",
        "93.184.216.34, 93.184.216.35",
        "93.184.216.34:443",
        "",
        "10.0.0.5",
        "192.168.1.9",
        "fd00::1",
    ] {
        let mut browser = harness.browser_at(address);

        assert_eq!(browser.status().await.status, StatusCode::FORBIDDEN, "{address}");
    }

    let two = harness
        .browser()
        .request(Method::GET, "/api/status", vec![header("x-forwarded-for", "93.184.216.35")], "")
        .await;

    assert_eq!(two.status, StatusCode::FORBIDDEN);
}
