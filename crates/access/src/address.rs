//! Address: the canonical public IP accit observed for a request (`GLOSSARY.md`).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(IpAddr);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    V4,
    V6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("no X-Forwarded-For value")]
    Missing,
    #[error("more than one X-Forwarded-For value")]
    Ambiguous,
    #[error("malformed address")]
    Malformed,
}

/// IPv4 ranges that are never a client's public address (RFC 6890 and successors).
const V4_SPECIAL: [(Ipv4Addr, u8); 16] = [
    (Ipv4Addr::UNSPECIFIED, 8),
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    (Ipv4Addr::new(100, 64, 0, 0), 10),
    (Ipv4Addr::new(127, 0, 0, 0), 8),
    (Ipv4Addr::new(169, 254, 0, 0), 16),
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    (Ipv4Addr::new(192, 0, 0, 0), 24),
    (Ipv4Addr::new(192, 0, 2, 0), 24),
    (Ipv4Addr::new(192, 88, 99, 0), 24),
    (Ipv4Addr::new(192, 168, 0, 0), 16),
    (Ipv4Addr::new(198, 18, 0, 0), 15),
    (Ipv4Addr::new(198, 51, 100, 0), 24),
    (Ipv4Addr::new(203, 0, 113, 0), 24),
    (Ipv4Addr::new(224, 0, 0, 0), 4),
    (Ipv4Addr::new(240, 0, 0, 0), 4),
    (Ipv4Addr::BROADCAST, 32),
];

/// Global unicast (`2000::/3`) is the only public IPv6 space; these parts of it are not.
const V6_GLOBAL: (Ipv6Addr, u8) = (Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3);
const V6_SPECIAL: [(Ipv6Addr, u8); 3] = [
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
];

pub(crate) fn v4_mask(prefix: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0)
}

pub(crate) fn v6_mask(prefix: u8) -> u128 {
    u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0)
}

fn v4_in(address: Ipv4Addr, (network, prefix): (Ipv4Addr, u8)) -> bool {
    let mask = v4_mask(prefix);

    u32::from(address) & mask == u32::from(network) & mask
}

fn v6_in(address: Ipv6Addr, (network, prefix): (Ipv6Addr, u8)) -> bool {
    let mask = v6_mask(prefix);

    u128::from(address) & mask == u128::from(network) & mask
}

impl Address {
    /// The values of every `X-Forwarded-For` header on the request. Traefik sets exactly one
    /// value; anything else (none, several, a list) is ambiguous and rejected.
    pub fn from_forwarded_for<'a>(values: impl IntoIterator<Item = &'a [u8]>) -> Result<Self, AddressError> {
        let mut values = values.into_iter();
        let value = values.next().ok_or(AddressError::Missing)?;

        if values.next().is_some() {
            return Err(AddressError::Ambiguous);
        }

        let text = std::str::from_utf8(value).map_err(|_| AddressError::Malformed)?;

        if text.contains(',') {
            return Err(AddressError::Ambiguous);
        }

        text.parse()
    }

    pub fn ip(self) -> IpAddr {
        self.0
    }

    pub fn family(self) -> Family {
        match self.0 {
            IpAddr::V4(_) => Family::V4,
            IpAddr::V6(_) => Family::V6,
        }
    }

    pub fn is_public(self) -> bool {
        match self.0 {
            IpAddr::V4(v4) => !V4_SPECIAL.iter().any(|range| v4_in(v4, *range)),
            IpAddr::V6(v6) => v6_in(v6, V6_GLOBAL) && !V6_SPECIAL.iter().any(|range| v6_in(v6, *range)),
        }
    }
}

impl From<IpAddr> for Address {
    /// IPv4-mapped IPv6 becomes IPv4, so one client has one Address.
    fn from(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V6(v6) => Self(v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4)),
            IpAddr::V4(_) => Self(ip),
        }
    }
}

impl FromStr for Address {
    type Err = AddressError;

    /// Exactly an IP: no whitespace, port, brackets, zone or prefix length.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<IpAddr>().map(Self::from).map_err(|_| AddressError::Malformed)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
