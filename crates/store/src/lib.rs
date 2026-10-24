//! Grants (`docs/spec.md` sections 6 and 7): an in-memory map that verification reads, kept in step
//! with the database by one serialized mutation path.

mod audit;
mod database;
mod time;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use access::Address;
use sqlx::SqliteConnection;
use tokio::sync::Mutex;

pub use audit::{Action, AuditEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub grant_duration: Duration,
    pub max_grants: u32,
    pub audit_retention: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantStatus {
    None,
    Active(SystemTime),
    /// Kept until audit retention passes.
    Expired(SystemTime),
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("grant database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("grant database migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("time out of range")]
    Time,
    #[error("stored address is invalid")]
    Address(#[from] access::AddressError),
    #[error("stored audit action `{0}` is unknown")]
    Action(String),
    #[error("grant mutation did not finish")]
    Interrupted,
}

#[derive(Debug, thiserror::Error)]
pub enum UnlockError {
    #[error("maximum number of active grants reached")]
    Full,
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug)]
struct Inner {
    limits: Limits,
    active: RwLock<HashMap<Address, SystemTime>>,
    /// The one mutation path: every change to `active` or the database holds this lock.
    database: Mutex<SqliteConnection>,
}

/// Cheap to clone; all clones share one state.
#[derive(Clone, Debug)]
pub struct Grants(Arc<Inner>);

impl Grants {
    /// Ready to serve once this returns: unexpired Grants are loaded and validated.
    /// `None` keeps everything in memory, lost on restart.
    pub async fn open(path: Option<&Path>, limits: Limits, now: SystemTime) -> Result<Self, StoreError> {
        let mut connection = database::connect(path).await?;
        let active = database::unexpired(&mut connection, now).await?;

        Ok(Self(Arc::new(Inner { limits, active: RwLock::new(active), database: Mutex::new(connection) })))
    }

    /// The expiry of the Address's Grant, if it is still in the future. Never mutates.
    pub fn active_until(&self, address: Address, now: SystemTime) -> Option<SystemTime> {
        let active = self.0.active.read().unwrap_or_else(PoisonError::into_inner);

        active.get(&address).copied().filter(|expiry| *expiry > now)
    }

    /// Creates or renews the Grant to `now + grant_duration`; visible only once committed.
    pub async fn unlock(&self, address: Address, now: SystemTime) -> Result<SystemTime, UnlockError> {
        let inner = Arc::clone(&self.0);

        // Spawned, so a caller that goes away cannot cut a mutation in half.
        tokio::spawn(async move { inner.unlock(address, now).await })
            .await
            .map_err(|_| StoreError::Interrupted)?
    }

    /// Removes the Grant from memory first, so it stops passing even if the write then fails.
    pub async fn revoke(&self, address: Address, now: SystemTime) -> Result<(), StoreError> {
        let inner = Arc::clone(&self.0);

        tokio::spawn(async move { inner.revoke(address, now).await })
            .await
            .map_err(|_| StoreError::Interrupted)?
    }

    /// Deletes Grants and audit events older than audit retention.
    pub async fn clean_up(&self, now: SystemTime) -> Result<(), StoreError> {
        let inner = Arc::clone(&self.0);

        tokio::spawn(async move { inner.clean_up(now).await }).await.map_err(|_| StoreError::Interrupted)?
    }

    pub async fn status(&self, address: Address, now: SystemTime) -> Result<GrantStatus, StoreError> {
        let mut connection = self.0.database.lock().await;

        if let Some(expiry) = self.active_until(address, now) {
            return Ok(GrantStatus::Active(expiry));
        }

        let stored = database::expiry(&mut connection, address).await?;

        Ok(stored.filter(|expiry| *expiry <= now).map_or(GrantStatus::None, GrantStatus::Expired))
    }

    /// Runs [`Self::clean_up`] every `period`, forever. A failed run is logged and retried next time.
    pub async fn clean_up_every(self, period: Duration) {
        let mut ticks = tokio::time::interval(period);

        loop {
            ticks.tick().await;
            self.clean_up_logged().await;
        }
    }

    async fn clean_up_logged(&self) {
        if let Err(error) = self.clean_up(SystemTime::now()).await {
            tracing::warn!(%error, "grant cleanup failed");
        }
    }

    /// Oldest first.
    pub async fn audit_events(&self) -> Result<Vec<AuditEvent>, StoreError> {
        database::audit_events(&mut *self.0.database.lock().await).await
    }
}

impl Inner {
    async fn unlock(&self, address: Address, now: SystemTime) -> Result<SystemTime, UnlockError> {
        let expiry = now.checked_add(self.limits.grant_duration).ok_or(StoreError::Time)?;
        let mut connection = self.database.lock().await;

        self.admit(address, now)?;
        database::unlock(&mut connection, address, expiry, now).await?;
        self.active.write().unwrap_or_else(PoisonError::into_inner).insert(address, expiry);

        Ok(expiry)
    }

    /// Renewal always passes; a new Address only below the cap, counting unexpired Grants.
    fn admit(&self, address: Address, now: SystemTime) -> Result<(), UnlockError> {
        let mut active = self.active.write().unwrap_or_else(PoisonError::into_inner);
        active.retain(|_, expiry| *expiry > now);
        let full = usize::try_from(self.limits.max_grants).is_ok_and(|max| active.len() >= max);

        if full && !active.contains_key(&address) {
            return Err(UnlockError::Full);
        }

        Ok(())
    }

    async fn revoke(&self, address: Address, now: SystemTime) -> Result<(), StoreError> {
        let mut connection = self.database.lock().await;

        self.active.write().unwrap_or_else(PoisonError::into_inner).remove(&address);
        database::revoke(&mut connection, address, now).await
    }

    async fn clean_up(&self, now: SystemTime) -> Result<(), StoreError> {
        let cutoff = now.checked_sub(self.limits.audit_retention).ok_or(StoreError::Time)?;
        let mut connection = self.database.lock().await;

        self.active.write().unwrap_or_else(PoisonError::into_inner).retain(|_, expiry| *expiry > now);
        database::delete_before(&mut connection, cutoff).await
    }
}
