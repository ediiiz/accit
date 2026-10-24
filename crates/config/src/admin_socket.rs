use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const FILE_NAME: &str = "admin.sock";

/// `admin_socket`, else `admin.sock` in the data directory, the one holding `database`.
pub(crate) fn resolve(admin_socket: Option<PathBuf>, database: Option<&Path>) -> Option<PathBuf> {
    admin_socket.or_else(|| database.map(|path| path.with_file_name(FILE_NAME)))
}

/// Only the keys that place the socket; everything else is ignored, not validated.
#[derive(Deserialize)]
struct Placement {
    admin_socket: Option<PathBuf>,
    database: Option<PathBuf>,
}

/// The admin socket named by `yaml`, read without validating the rest: `accit admin` must reach
/// the daemon to hear why a file is invalid.
pub fn admin_socket(yaml: &str) -> Result<Option<PathBuf>> {
    let placement: Placement = serde_saphyr::from_str(yaml).context("invalid configuration")?;

    Ok(resolve(placement.admin_socket, placement.database.as_deref()))
}
