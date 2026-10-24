//! accit's domain: Addresses, hostnames, the Policy and the gate decision. No I/O.

mod address;
mod gate;
mod hostname;
mod policy;

pub use address::{Address, AddressError, Family};
pub use gate::{Gate, Verdict};
pub use hostname::{Hostname, HostnameError};
pub use policy::Policy;
