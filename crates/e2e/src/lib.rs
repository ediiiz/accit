//! End-to-end suite against the Compose harness (`harness/`), run on demand by `scripts/e2e`.
//! Nothing here runs in `scripts/ci`: the scenarios are `#[ignore]`d without the stack.

pub mod client;
pub mod reply;
pub mod stack;

pub use client::{Call, Outcome};
pub use stack::{Session, Stack};

pub const PORTAL: &str = "portal.accit.test";
pub const GATED: &str = "gated.accit.test";
pub const OPEN: &str = "open.accit.test";

/// Source addresses of the client container (`harness/compose.yml`).
pub const ADDRESS_A: &str = "10.89.10.11";
pub const ADDRESS_B: &str = "10.89.10.12";
