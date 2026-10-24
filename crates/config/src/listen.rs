use std::net::{Ipv4Addr, SocketAddr};

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Listen {
    #[serde(default = "Listen::default_portal")]
    pub portal: SocketAddr,
    #[serde(default = "Listen::default_verify")]
    pub verify: SocketAddr,
}

impl Listen {
    fn default_portal() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080))
    }

    fn default_verify() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8081))
    }
}

impl Default for Listen {
    fn default() -> Self {
        Self { portal: Self::default_portal(), verify: Self::default_verify() }
    }
}
