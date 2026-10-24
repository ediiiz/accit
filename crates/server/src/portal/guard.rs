//! What a mutation must prove before it runs (docs/spec.md section 8): the exact `Origin`, JSON,
//! the session's CSRF token, a body that says nothing, and budget. Only a well-formed mutation
//! spends budget.

use access::Address;
use axum::body::Body;
use axum::http::{HeaderMap, header};

use super::Portal;
use super::error::ApiError;
use super::session::{self, tokens_match};

const MAX_BODY: usize = 1024;

/// A mutation that passed every check: whose it is, and the token to hand back with the result.
#[derive(Debug)]
pub(super) struct Checked {
    pub address: Address,
    pub csrf_token: String,
}

/// Exactly one `Origin`, equal to the configured one. Without a configured origin nothing matches.
fn origin_matches(headers: &HeaderMap, expected: Option<&str>) -> bool {
    let mut values = headers.get_all(header::ORIGIN).iter();

    match (values.next(), values.next(), expected) {
        (Some(sent), None, Some(expected)) => sent.as_bytes() == expected.as_bytes(),
        _ => false,
    }
}

fn is_json(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();

    match (values.next().and_then(|value| value.to_str().ok()), values.next()) {
        (Some(sent), None) => {
            sent.split(';').next().is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        }
        _ => false,
    }
}

/// Only `{}`: the Address comes from the connection, never from the body.
async fn empty_object(body: Body) -> Result<(), ApiError> {
    let bytes = axum::body::to_bytes(body, MAX_BODY).await.map_err(|_| ApiError::TooLarge)?;
    let object: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::BadRequest)?;

    if object.is_empty() { Ok(()) } else { Err(ApiError::BadRequest) }
}

impl Portal {
    pub(super) async fn checked(&self, headers: &HeaderMap, body: Body) -> Result<Checked, ApiError> {
        let address = self.requester(headers)?;
        let now = self.0.clock.now();

        if !origin_matches(headers, self.0.origin.as_deref()) {
            return Err(ApiError::Forbidden);
        }

        if !is_json(headers) {
            return Err(ApiError::UnsupportedMediaType);
        }

        let csrf_token = self.csrf_token(headers, now)?;

        empty_object(body).await?;
        self.0.limiter.admit(address, now).map_err(ApiError::RateLimited)?;

        Ok(Checked { address, csrf_token })
    }

    /// The session's token, when the request proves it holds it.
    fn csrf_token(&self, headers: &HeaderMap, now: std::time::SystemTime) -> Result<String, ApiError> {
        let expected = session::cookie_id(headers).and_then(|id| self.0.sessions.token(id, now));
        let mut sent = headers.get_all("x-csrf-token").iter();

        match (expected, sent.next(), sent.next()) {
            (Some(expected), Some(sent), None)
                if sent.to_str().is_ok_and(|sent| tokens_match(sent, &expected)) =>
            {
                Ok(expected)
            }
            _ => Err(ApiError::Forbidden),
        }
    }
}
