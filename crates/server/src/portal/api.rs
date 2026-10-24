//! `GET /api/status`, `POST /api/unlock`, `POST /api/revoke`. Every answer is the same JSON, the
//! state of the requesting Address.

use std::time::{SystemTime, UNIX_EPOCH};

use access::{Address, Family};
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use store::{GrantStatus, StoreError, UnlockError};

use super::error::ApiError;
use super::session::{self, COOKIE, LIFETIME};
use super::{App, Portal};

#[derive(Debug, Serialize)]
struct Status<'a> {
    /// `none`, `active` or `expired`.
    state: &'static str,
    /// Unix seconds; `null` without a Grant.
    expiry: Option<u64>,
    /// Unix seconds.
    server_time: u64,
    address: String,
    family: &'static str,
    apps: &'a [App],
    title: &'a str,
    default_language: &'static str,
    csrf_token: String,
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

impl Portal {
    fn status_body(
        &self,
        address: Address,
        grant: GrantStatus,
        now: SystemTime,
        csrf_token: String,
    ) -> Status<'_> {
        let (state, expiry) = match grant {
            GrantStatus::None => ("none", None),
            GrantStatus::Active(expiry) => ("active", Some(expiry)),
            GrantStatus::Expired(expiry) => ("expired", Some(expiry)),
        };

        Status {
            state,
            expiry: expiry.map(unix_seconds),
            server_time: unix_seconds(now),
            address: address.to_string(),
            family: match address.family() {
                Family::V4 => "ipv4",
                Family::V6 => "ipv6",
            },
            apps: &self.0.apps,
            title: &self.0.title,
            default_language: self.0.default_language,
            csrf_token,
        }
    }
}

fn unavailable(error: &StoreError) -> ApiError {
    tracing::warn!(%error, "grant store failed");

    ApiError::Unavailable
}

fn session_cookie(id: &str) -> Option<HeaderValue> {
    let attributes = format!("Path=/; Max-Age={}; Secure; HttpOnly; SameSite=Strict", LIFETIME.as_secs());

    HeaderValue::try_from(format!("{COOKIE}={id}; {attributes}")).ok()
}

pub(super) async fn status(State(portal): State<Portal>, headers: HeaderMap) -> Result<Response, ApiError> {
    let address = portal.requester(&headers)?;
    let now = portal.0.clock.now();
    let session = portal
        .0
        .sessions
        .resume_or_start(session::cookie_id(&headers), now)
        .map_err(|_| ApiError::Unavailable)?;
    let grant = portal.0.grants.status(address, now).await.map_err(|error| unavailable(&error))?;
    let body = Json(portal.status_body(address, grant, now, session.token)).into_response();

    Ok(match session.started.as_deref().and_then(session_cookie) {
        Some(cookie) => ([(header::SET_COOKIE, cookie)], body).into_response(),
        None => body,
    })
}

pub(super) async fn unlock(
    State(portal): State<Portal>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let checked = portal.checked(&headers, body).await?;
    let now = portal.0.clock.now();
    let expiry = portal.0.grants.unlock(checked.address, now).await.map_err(|error| match error {
        UnlockError::Full => ApiError::Full,
        UnlockError::Store(error) => unavailable(&error),
    })?;

    Ok(Json(portal.status_body(checked.address, GrantStatus::Active(expiry), now, checked.csrf_token))
        .into_response())
}

impl Portal {
    /// The Grant is already gone from memory when the write fails, but its row may survive a
    /// restart: fail closed until someone looks.
    async fn revoke_durably(&self, address: Address, now: SystemTime) -> Result<(), ApiError> {
        self.0.grants.revoke(address, now).await.map_err(|error| {
            tracing::error!(%error, "grant revoke was not written");

            ApiError::Unavailable
        })
    }
}

pub(super) async fn revoke(
    State(portal): State<Portal>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let checked = portal.checked(&headers, body).await?;
    let now = portal.0.clock.now();

    portal.revoke_durably(checked.address, now).await?;

    let grant = portal.0.grants.status(checked.address, now).await.map_err(|error| unavailable(&error))?;

    Ok(Json(portal.status_body(checked.address, grant, now, checked.csrf_token)).into_response())
}
