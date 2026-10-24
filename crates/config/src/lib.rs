//! accit configuration (`/config/accit.yaml`), read at startup and on reload. Unknown keys are errors.

mod apps;
mod config;
mod duration;
mod listen;
mod protection;
mod token;

use std::path::PathBuf;

pub use apps::AppLink;
pub use config::{Config, Language, Profile};
pub use listen::Listen;
pub use token::ProxyToken;

pub const DEFAULT_PATH: &str = "/config/accit.yaml";

/// `ACCIT_CONFIG`, else [`DEFAULT_PATH`].
pub fn path_from_env() -> PathBuf {
    std::env::var_os("ACCIT_CONFIG").map_or_else(|| PathBuf::from(DEFAULT_PATH), PathBuf::from)
}
