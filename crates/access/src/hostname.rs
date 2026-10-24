//! Canonical hostnames: what Policy entries and `X-Forwarded-Host` are compared as.

use std::fmt;
use std::net::IpAddr;

/// Lowercase ASCII (IDNA) letters, digits and hyphens; no port, no trailing dot.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hostname(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HostnameError {
    #[error("empty hostname")]
    Empty,
    #[error("hostname is not valid UTF-8")]
    NotUtf8,
    #[error("hostname contains a character outside letters, digits, '-' and '.'")]
    InvalidCharacter,
    #[error("invalid port")]
    InvalidPort,
    #[error("an IP address is not a hostname")]
    IpLiteral,
    #[error("invalid label")]
    InvalidLabel,
    #[error("hostname longer than 253 characters")]
    TooLong,
}

fn valid_label(label: &str) -> bool {
    (1..=63).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// WHATWG "ends in a number": all decimal digits, or `0x` followed only by hex digits.
fn is_numeric_label(label: &str) -> bool {
    let hex = label.strip_prefix("0x").or_else(|| label.strip_prefix("0X"));

    match hex {
        Some(digits) => digits.bytes().all(|byte| byte.is_ascii_hexdigit()),
        None => label.bytes().all(|byte| byte.is_ascii_digit()),
    }
}

fn validate_ascii(name: &str) -> Result<(), HostnameError> {
    if name.len() > 253 {
        return Err(HostnameError::TooLong);
    }

    if !name.split('.').all(valid_label) {
        return Err(HostnameError::InvalidLabel);
    }

    // A numeric last label is an IPv4 literal in disguise (`1.2.3`, `0x7f.1`, `0x7f000001`).
    if name.rsplit('.').next().is_some_and(is_numeric_label) {
        return Err(HostnameError::IpLiteral);
    }

    Ok(())
}

impl Hostname {
    /// A hostname as written in the Policy: no port, at most one trailing dot.
    pub fn parse(input: &str) -> Result<Self, HostnameError> {
        let name = input.strip_suffix('.').unwrap_or(input);

        if name.is_empty() {
            return Err(HostnameError::Empty);
        }

        if name.parse::<IpAddr>().is_ok() {
            return Err(HostnameError::IpLiteral);
        }

        if !name.chars().all(|character| character.is_alphanumeric() || character == '-' || character == '.')
        {
            return Err(HostnameError::InvalidCharacter);
        }

        let ascii = idna::domain_to_ascii_strict(name).map_err(|_| HostnameError::InvalidLabel)?;

        validate_ascii(&ascii)?;

        Ok(Self(ascii))
    }

    /// Traefik's `X-Forwarded-Host`: the request's Host, possibly with a port.
    pub fn from_forwarded_host(value: &[u8]) -> Result<Self, HostnameError> {
        let text = std::str::from_utf8(value).map_err(|_| HostnameError::NotUtf8)?;
        let name = match text.rsplit_once(':') {
            Some((name, port)) => {
                if name.contains(':') || port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(HostnameError::InvalidPort);
                }

                port.parse::<u16>().map_err(|_| HostnameError::InvalidPort)?;
                name
            }
            None => text,
        };

        Self::parse(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
