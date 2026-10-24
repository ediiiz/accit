//! Grant administration (`docs/spec.md` section 11), on the same mutation path as Unlock and Revoke.

use std::io;
use std::sync::PoisonError;
use std::time::SystemTime;

use access::Address;
use tokio::time::Instant;

use crate::{Grant, Grants, Inner, StoreError, database};

/// What `accit admin recover` does with the stored Grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// Revokes them all: a Revoke that was not saved cannot come back.
    DiscardGrants,
    /// Trusts them, including any whose Revoke was not saved.
    KeepGrants,
}

#[derive(Debug, thiserror::Error)]
pub enum RecoverError {
    #[error("not failed closed; nothing to recover")]
    NotFailedClosed,
    #[error("removing the fail-closed marker: {0}")]
    Marker(#[source] io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Grants {
    /// Active Grants and their expiry, by Address.
    pub fn list(&self, now: SystemTime) -> Vec<(Address, SystemTime)> {
        let active = self.0.active.read().unwrap_or_else(PoisonError::into_inner);
        let mut listed: Vec<_> = active
            .iter()
            .filter(|(_, grant)| grant.valid(now))
            .map(|(address, grant)| (*address, grant.expiry))
            .collect();
        listed.sort_unstable();

        listed
    }

    /// Revokes every active Grant and returns how many; memory first, so a failed write fails closed as
    /// [`Self::revoke`] does.
    pub async fn revoke_all(&self, now: SystemTime) -> Result<u64, StoreError> {
        self.mutate(move |inner| async move { inner.revoke_all(now).await }).await
    }

    /// Leaves [`crate::Integrity::FailedClosed`]: applies `recovery`, then removes the marker. Any error
    /// leaves accit failed closed.
    pub async fn recover(&self, recovery: Recovery, now: SystemTime) -> Result<(), RecoverError> {
        self.mutate(move |inner| async move { inner.recover(recovery, now).await }).await
    }
}

impl Inner {
    async fn revoke_all(&self, now: SystemTime) -> Result<u64, StoreError> {
        let mut connection = self.database.lock().await;

        self.active.write().unwrap_or_else(PoisonError::into_inner).clear();
        let unsaved = self.integrity.unsaved();
        let revoked = database::revoke_all(&mut connection, now).await?;
        unsaved.saved();

        Ok(revoked)
    }

    async fn recover(&self, recovery: Recovery, now: SystemTime) -> Result<(), RecoverError> {
        let mut connection = self.database.lock().await;

        if !self.integrity.is_failed_closed() {
            return Err(RecoverError::NotFailedClosed);
        }
        match recovery {
            Recovery::DiscardGrants => {
                self.active.write().unwrap_or_else(PoisonError::into_inner).clear();
                database::revoke_all(&mut connection, now).await?;
            }
            Recovery::KeepGrants => {
                let stored = database::unexpired(&mut connection, now).await?;
                let loaded = Instant::now();
                let mut active = self.active.write().unwrap_or_else(PoisonError::into_inner);
                for (address, expiry) in stored {
                    active.entry(address).or_insert_with(|| Grant::stored(expiry, now, loaded));
                }
            }
        }

        self.integrity.recover().map_err(RecoverError::Marker)
    }
}
