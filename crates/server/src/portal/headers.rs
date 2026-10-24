//! Headers on every Portal response, the Traefik-level refusals included.

use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

const CSP: &str = "default-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; \
                   frame-ancestors 'none'; form-action 'self'";

/// `'sha256-…'` source expressions for a page's inline scripts: the static build inlines its bootstrap.
#[derive(Clone, Debug)]
pub struct InlineScripts(Vec<String>);

impl InlineScripts {
    pub fn of(html: &str) -> Self {
        Self(html.split("<script").skip(1).filter_map(inline_body).map(hash).collect())
    }
}

/// The text of a script element, given the markup after its `<script`; `None` for `src=` scripts.
fn inline_body(rest: &str) -> Option<&str> {
    let (attributes, body) = rest.split_once('>')?;

    if attributes.contains("src=") {
        return None;
    }

    body.split_once("</script>").map(|(text, _)| text)
}

fn hash(text: &str) -> String {
    format!("'sha256-{}'", STANDARD.encode(Sha256::digest(text.as_bytes())))
}

fn policy(scripts: Option<&InlineScripts>) -> String {
    match scripts {
        Some(InlineScripts(hashes)) if !hashes.is_empty() => {
            format!("{CSP}; script-src 'self' {}", hashes.join(" "))
        }
        _ => CSP.to_owned(),
    }
}

pub(super) async fn secure(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let policy = policy(response.extensions().get::<InlineScripts>());
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&policy).unwrap_or(HeaderValue::from_static(CSP)),
    );

    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    // Hashed assets and the other files set their own policy; everything else is never stored.
    headers.entry(header::CACHE_CONTROL).or_insert(HeaderValue::from_static("no-store"));

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inline_script_is_allowed_by_its_hash() {
        let scripts = InlineScripts::of("<head></head><body><script>alert(1)</script></body>");

        assert_eq!(scripts.0, ["'sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI='"]);
    }

    #[test]
    fn whitespace_is_part_of_the_hash() {
        let scripts = InlineScripts::of("<script> a </script>");

        assert_eq!(scripts.0, ["'sha256-y/fzAATzZnywk7PHtVFpqQ4/AET26j/5PT91Qnpy43c='"]);
    }

    #[test]
    fn external_scripts_need_no_hash() {
        assert!(
            InlineScripts::of(r#"<script src="a.js"></script><script type="module" src="b.js"></script>"#)
                .0
                .is_empty()
        );
    }

    #[test]
    fn a_page_policy_names_its_hashes_and_keeps_the_rest() {
        let scripts = InlineScripts(vec!["'sha256-x='".into(), "'sha256-y='".into()]);
        let policy = policy(Some(&scripts));

        assert!(
            policy.split("; ").any(|part| part == "script-src 'self' 'sha256-x=' 'sha256-y='"),
            "{policy}"
        );
        assert!(policy.split("; ").any(|part| part == "default-src 'self'"), "{policy}");
    }

    #[test]
    fn without_inline_scripts_the_policy_adds_nothing() {
        assert!(!policy(None).contains("script-src"));
    }
}
