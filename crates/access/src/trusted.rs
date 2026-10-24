//! Trusted networks: private ranges whose Addresses pass the gate without a Grant (docs/spec.md section 5).

use std::collections::BTreeSet;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::Address;
use crate::address::{v4_mask, v6_mask};

/// An IP range. Host bits are cleared on parse, so `172.18.0.1/16` is `172.18.0.0/16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("not an address range: expected `<ip>/<prefix length>`")]
pub struct CidrError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TrustedError {
    #[error("{0} is not a private, unique-local or CGNAT range")]
    NotPrivate(Cidr),
    #[error("{network} overlaps the Docker network {docker}")]
    OverlapsDocker { network: Cidr, docker: Cidr },
}

/// The only ranges a Trusted network may lie inside: `10/8`, `172.16/12`, `192.168/16`, `100.64/10`, `fc00::/7`.
const ALLOWED: [Cidr; 5] = [
    Cidr { network: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)), prefix: 8 },
    Cidr { network: IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0)), prefix: 12 },
    Cidr { network: IpAddr::V4(Ipv4Addr::new(192, 168, 0, 0)), prefix: 16 },
    Cidr { network: IpAddr::V4(Ipv4Addr::new(100, 64, 0, 0)), prefix: 10 },
    Cidr { network: IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0)), prefix: 7 },
];

impl Cidr {
    pub fn contains(&self, address: Address) -> bool {
        self.contains_ip(address.ip())
    }

    fn contains_ip(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                u32::from(ip) & v4_mask(self.prefix) == u32::from(network)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                u128::from(ip) & v6_mask(self.prefix) == u128::from(network)
            }
            _ => false,
        }
    }

    /// Every address of `other` is in `self`.
    fn covers(&self, other: &Self) -> bool {
        self.prefix <= other.prefix && self.contains_ip(other.network)
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.covers(other) || other.covers(self)
    }
}

impl FromStr for Cidr {
    type Err = CidrError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (ip, prefix) = text.split_once('/').ok_or(CidrError)?;
        let prefix: u8 = prefix.parse().map_err(|_| CidrError)?;
        let network = match ip.parse::<IpAddr>().map_err(|_| CidrError)? {
            IpAddr::V4(ip) if prefix <= 32 => IpAddr::V4(Ipv4Addr::from(u32::from(ip) & v4_mask(prefix))),
            IpAddr::V6(ip) if prefix <= 128 => IpAddr::V6(Ipv6Addr::from(u128::from(ip) & v6_mask(prefix))),
            _ => return Err(CidrError),
        };

        Ok(Self { network, prefix })
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

/// Ranges declared inside the home, minus single excluded Addresses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustedNetworks {
    networks: Vec<Cidr>,
    excluded: BTreeSet<Address>,
}

impl TrustedNetworks {
    /// Rejects a range outside the private spaces, or one overlapping a Docker network.
    pub fn new(
        networks: impl IntoIterator<Item = Cidr>,
        excluded: impl IntoIterator<Item = Address>,
        docker: &[Cidr],
    ) -> Result<Self, TrustedError> {
        let networks: BTreeSet<Cidr> = networks.into_iter().collect();

        for network in &networks {
            if !ALLOWED.iter().any(|allowed| allowed.covers(network)) {
                return Err(TrustedError::NotPrivate(*network));
            }

            if let Some(docker) = docker.iter().find(|docker| docker.overlaps(network)) {
                return Err(TrustedError::OverlapsDocker { network: *network, docker: *docker });
            }
        }

        Ok(Self { networks: networks.into_iter().collect(), excluded: excluded.into_iter().collect() })
    }

    pub fn contains(&self, address: Address) -> bool {
        !self.excluded.contains(&address) && self.networks.iter().any(|network| network.contains(address))
    }
}
