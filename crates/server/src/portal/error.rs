use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// A refused API call. Bodies are the status text only: no detail leaves through Traefik.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ApiError {
    Forbidden,
    UnsupportedMediaType,
    TooLarge,
    BadRequest,
    /// No room for a new Address.
    Full,
    /// Retry after this many seconds.
    RateLimited(u64),
    Unavailable,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Full => StatusCode::CONFLICT,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        };
        let mut response = (status, status.canonical_reason().unwrap_or_default()).into_response();

        if let Self::RateLimited(seconds) = self
            && let Ok(value) = HeaderValue::try_from(seconds.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }

        response
    }
}

pub(super) async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}
