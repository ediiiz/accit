//! accit's domain: Addresses, hostnames and (later) Grants and the gate decision. No I/O.

mod address;
mod hostname;

pub use address::{Address, AddressError, Family};
pub use hostname::{Hostname, HostnameError};
