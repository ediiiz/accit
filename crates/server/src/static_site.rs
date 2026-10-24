//! The built Portal (`apps/web/build`), embedded in release builds, read from disk in debug.
//! Prerendered, so only exact files and directory indexes exist; everything else is 404.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../apps/web/build"]
#[allow_missing = true]
struct Site;

fn cache_control(path: &str) -> &'static str {
    // Content-hashed by Vite: a new build gets new names.
    if path.starts_with("_app/immutable/") {
        return "public, max-age=31536000, immutable";
    }

    // Revalidated, so a deploy reaches open tabs.
    if extension(path) == "html" || path == "_app/version.json" {
        return "no-cache";
    }

    "public, max-age=3600"
}

/// An exact file or a page (`about` → `about.html`, `docs/` → `docs/index.html`).
fn resolve(path: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let page = if path.is_empty() || path.ends_with('/') {
        format!("{path}index.html")
    } else {
        format!("{path}.html")
    };

    [path.to_owned(), page].into_iter().find(|candidate| !candidate.is_empty() && exists(candidate))
}

/// Precompressed siblings (`x.br`, `x.gz`) from adapter-static, when the client accepts them.
fn encoded(path: &str, headers: &HeaderMap) -> Option<(&'static str, rust_embed::EmbeddedFile)> {
    let accepted =
        headers.get(header::ACCEPT_ENCODING).and_then(|value| value.to_str().ok()).unwrap_or_default();
    let accepts = |name: &str| accepted.split(',').any(|entry| entry.trim().split(';').next() == Some(name));

    [("br", "br"), ("gzip", "gz")]
        .into_iter()
        .filter(|(name, _)| accepts(name))
        .find_map(|(name, extension)| Site::get(&format!("{path}.{extension}")).map(|file| (name, file)))
}

pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let Some((route, file)) = resolve(path, |file| Site::get(file).is_some())
        .and_then(|route| Site::get(&route).map(|file| (route, file)))
    else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };
    let mime = mime_type(&route);
    let (encoding, body) = match encoded(&route, &headers) {
        Some((encoding, file)) => (Some(encoding), file.data),
        None => (None, file.data),
    };
    let mut response = Response::new(Body::from(body.into_owned()));
    let response_headers = response.headers_mut();

    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control(&route)));
    response_headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));

    if let Some(encoding) = encoding {
        response_headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
    }

    response
}

fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or_default();

    name.rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase()).unwrap_or_default()
}

fn mime_type(path: &str) -> &'static str {
    match extension(path).as_str() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_in(path: &str) -> Option<String> {
        resolve(path, |file| {
            ["index.html", "about.html", "_app/immutable/app.js", "favicon.png"].contains(&file)
        })
    }

    #[test]
    fn pages_resolve_to_their_html() {
        assert_eq!(resolve_in("").as_deref(), Some("index.html"));
        assert_eq!(resolve_in("about").as_deref(), Some("about.html"));
    }

    #[test]
    fn anything_else_is_missing() {
        assert_eq!(resolve_in("verify"), None);
        assert_eq!(resolve_in("projects/123"), None);
        assert_eq!(resolve_in("missing.js"), None);
        assert_eq!(resolve_in("_app/immutable/app.js").as_deref(), Some("_app/immutable/app.js"));
    }

    #[test]
    fn hashed_assets_are_immutable_and_pages_revalidate() {
        assert_eq!(cache_control("_app/immutable/app.js"), "public, max-age=31536000, immutable");
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("_app/version.json"), "no-cache");
        assert_eq!(cache_control("favicon.png"), "public, max-age=3600");
    }
}
