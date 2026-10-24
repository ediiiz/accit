//! Fail-closed state (`docs/spec.md` section 7). A Revoke whose write failed may still pass after a
//! restart, so a marker in the data directory keeps accit failed closed until it is recovered.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tokio::sync::watch;

/// Its file name, next to the database.
pub const FAIL_CLOSED_MARKER: &str = "fail-closed";

const NOTE: &[u8] =
    b"A Revoke was not saved. Run `accit admin recover --discard-grants` or `--keep-grants`.\n";

/// Ordered from best to worst; within one process it only gets worse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Integrity {
    Sound,
    /// A Revoke may not be durable: no Grant can be trusted until recovery.
    FailedClosed,
    /// The marker could not be written either; the process must exit.
    Unrecoverable,
}

#[derive(Debug)]
pub(crate) struct Guard {
    /// `None` without a database: nothing survives a restart, so there is nothing to mark.
    marker: Option<PathBuf>,
    state: watch::Sender<Integrity>,
}

impl Guard {
    pub(crate) fn open(database: Option<&Path>) -> Self {
        let marker = database.map(|path| path.with_file_name(FAIL_CLOSED_MARKER));
        // A marker that cannot be checked counts as present.
        let present = marker.as_deref().is_some_and(|path| path.try_exists().unwrap_or(true));
        let initial = if present { marked() } else { Integrity::Sound };

        Self { marker, state: watch::channel(initial).0 }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<Integrity> {
        self.state.subscribe()
    }

    pub(crate) fn fail_closed(&self) {
        tracing::error!("revoke not saved; failed closed until recovered");
        let reached = match self.marker.as_deref().map_or(Ok(()), write) {
            Ok(()) => Integrity::FailedClosed,
            Err(error) => unwritable(&error),
        };

        self.state.send_modify(|state| *state = (*state).max(reached));
    }
}

fn marked() -> Integrity {
    tracing::error!("fail-closed marker present; unready until `accit admin recover`");

    Integrity::FailedClosed
}

fn unwritable(error: &io::Error) -> Integrity {
    tracing::error!(%error, "fail-closed marker not written");

    Integrity::Unrecoverable
}

fn write(path: &Path) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(NOTE)?;
    file.sync_all()?;

    // The new directory entry must be durable too.
    let directory = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    File::open(directory)?.sync_all()
}
