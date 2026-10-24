//! Hosts inside `https://` URLs: the Portal's `public_origin` and app links.

use access::Hostname;
use anyhow::{Context, Result, bail};

const DEFAULT_HTTPS_PORT: u16 = 443;

/// `host[:port]` as `(Hostname, port)`; the port is optional but must be a real one (1 to 65535) when present.
pub(crate) fn host_and_port(authority: &str) -> Result<(Hostname, Option<u16>)> {
    let (name, port) = match authority.rsplit_once(':') {
        Some((name, port)) => {
            let port = port
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .filter(|_| port.bytes().all(|byte| byte.is_ascii_digit()));

            (name, Some(port.context("invalid port")?))
        }
        None => (authority, None),
    };

    Ok((Hostname::parse(name)?, port))
}

/// The authority of an `https://` URL: everything before the first `/`, `?` or `#`.
pub(crate) fn https_authority(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;

    rest.split(['/', '?', '#']).next()
}

/// `https://<hostname>[:port][/]`, nothing else; returns the canonical origin and its hostname.
/// Browsers omit the default port from `Origin`, so the canonical origin does too.
pub(crate) fn public_origin(text: &str) -> Result<(String, Hostname)> {
    let Some(rest) = text.strip_prefix("https://") else {
        bail!("public_origin must start with https://, got `{text}`");
    };
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    let (host, port) = host_and_port(authority)
        .with_context(|| format!("public_origin `{text}` is not https://<hostname>"))?;
    let origin = match port.filter(|port| *port != DEFAULT_HTTPS_PORT) {
        Some(port) => format!("https://{host}:{port}"),
        None => format!("https://{host}"),
    };

    Ok((origin, host))
}
