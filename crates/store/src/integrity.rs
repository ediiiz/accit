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

/// Ordered from best to worst; only `accit admin recover` makes it better.
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

    /// Fails closed unless [`Unsaved::saved`] runs first.
    pub(crate) fn unsaved(&self) -> Unsaved<'_> {
        Unsaved(Some(self))
    }

    pub(crate) fn is_failed_closed(&self) -> bool {
        *self.state.borrow() == Integrity::FailedClosed
    }

    /// Removes the marker, then reports [`Integrity::Sound`]; on error nothing changes.
    pub(crate) fn recover(&self) -> io::Result<()> {
        if let Some(marker) = self.marker.as_deref() {
            remove(marker)?;
        }
        self.state.send_if_modified(|state| {
            let recovering = *state == Integrity::FailedClosed;
            if recovering {
                *state = Integrity::Sound;
            }
            recovering
        });

        Ok(())
    }

    fn fail_closed(&self) {
        tracing::error!("revoke not saved; failed closed until recovered");
        let reached = match self.marker.as_deref().map_or(Ok(()), write) {
            Ok(()) => Integrity::FailedClosed,
            Err(error) => unwritable(&error),
        };

        self.state.send_modify(|state| *state = (*state).max(reached));
    }
}

/// A Revoke's write in flight: dropped before [`Self::saved`] (an error, a panic or cancellation), it fails
/// closed.
pub(crate) struct Unsaved<'a>(Option<&'a Guard>);

impl Unsaved<'_> {
    pub(crate) fn saved(mut self) {
        self.0 = None;
    }
}

impl Drop for Unsaved<'_> {
    fn drop(&mut self) {
        if let Some(guard) = self.0 {
            guard.fail_closed();
        }
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

    sync_directory(path)
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => sync_directory(path),
    }
}

/// Makes the creation or removal of `path`'s directory entry durable.
fn sync_directory(path: &Path) -> io::Result<()> {
    let directory = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    File::open(directory)?.sync_all()
}
