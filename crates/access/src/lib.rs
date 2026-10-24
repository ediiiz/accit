//! accit's domain: Addresses, hostnames, the Policy, Trusted networks and the gate decision. No I/O.

mod address;
mod gate;
mod hostname;
mod policy;
mod trusted;

pub use address::{Address, AddressError, Family};
pub use gate::{Gate, Verdict};
pub use hostname::{Hostname, HostnameError};
pub use policy::Policy;
pub use trusted::{Cidr, CidrError, TrustedError, TrustedNetworks};
