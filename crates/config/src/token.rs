use std::fmt;

/// The shared secret Traefik sets in `X-Accit-Proxy-Token`. Deliberately not `Display`.
#[derive(Clone)]
pub struct ProxyToken(String);

impl ProxyToken {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProxyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyToken(<redacted>)")
    }
}
