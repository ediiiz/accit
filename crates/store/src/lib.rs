//! Grants (`docs/spec.md` sections 6 and 7): an in-memory map that verification reads, kept in step
//! with the database by one serialized mutation path.

mod audit;
mod database;
mod integrity;
#[cfg(test)]
mod interrupted;
mod time;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use access::Address;
use sqlx::SqliteConnection;
use tokio::sync::{Mutex, watch};
use tokio::time::Instant;

pub use audit::{Action, AuditEvent};
pub use integrity::{FAIL_CLOSED_MARKER, Integrity};

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

/// A Grant in memory: both times must lie in the future for it to pass.
#[derive(Clone, Copy, Debug)]
struct Grant {
    expiry: SystemTime,
    /// Set when the Grant is created or loaded, so a wall-clock jump cannot extend it.
    deadline: Instant,
}

impl Grant {
    fn valid(&self, now: SystemTime) -> bool {
        self.expiry > now && self.deadline > Instant::now()
    }
}

#[derive(Debug)]
struct Inner {
    limits: Limits,
    active: RwLock<HashMap<Address, Grant>>,
    /// The one mutation path: every change to `active` or the database holds this lock.
    database: Mutex<SqliteConnection>,
    integrity: integrity::Guard,
}

/// Cheap to clone; all clones share one state.
#[derive(Clone, Debug)]
pub struct Grants(Arc<Inner>);

impl Grants {
    /// Ready to serve once this returns: unexpired Grants are loaded and validated.
    /// `None` keeps everything in memory, lost on restart.
    pub async fn open(path: Option<&Path>, limits: Limits, now: SystemTime) -> Result<Self, StoreError> {
        let mut connection = database::connect(path).await?;
        let loaded = Instant::now();
        let active = database::unexpired(&mut connection, now)
            .await?
            .into_iter()
            .map(|(address, expiry)| {
                let remaining = expiry.duration_since(now).unwrap_or_default();
                (address, Grant { expiry, deadline: loaded + remaining })
            })
            .collect();

        let integrity = integrity::Guard::open(path);

        Ok(Self(Arc::new(Inner {
            limits,
            active: RwLock::new(active),
            database: Mutex::new(connection),
            integrity,
        })))
    }

    /// The expiry of the Address's Grant while its expiry and monotonic deadline both lie ahead. Never mutates.
    pub fn active_until(&self, address: Address, now: SystemTime) -> Option<SystemTime> {
        let active = self.0.active.read().unwrap_or_else(PoisonError::into_inner);

        active.get(&address).filter(|grant| grant.valid(now)).map(|grant| grant.expiry)
    }

    /// Creates or renews the Grant to `now + grant_duration`; visible only once committed.
    pub async fn unlock(&self, address: Address, now: SystemTime) -> Result<SystemTime, UnlockError> {
        self.mutate(move |inner| async move { inner.unlock(address, now).await }).await
    }

    /// [`Integrity::FailedClosed`] when a marker from an earlier failed Revoke is present at open.
    pub fn integrity(&self) -> watch::Receiver<Integrity> {
        self.0.integrity.subscribe()
    }

    /// Removes the Grant from memory first, so it stops passing even if the write then fails; a failed
    /// or interrupted write fails closed (see [`Self::integrity`]).
    pub async fn revoke(&self, address: Address, now: SystemTime) -> Result<(), StoreError> {
        self.revoke_task(move |inner| async move { inner.revoke(address, now).await }).await
    }

    /// A task that died may have left the Grant out of memory but in the database.
    async fn revoke_task<F>(&self, task: impl FnOnce(Arc<Inner>) -> F) -> Result<(), StoreError>
    where
        F: Future<Output = Result<(), StoreError>> + Send + 'static,
    {
        let result = self.mutate(task).await;

        if matches!(result, Err(StoreError::Interrupted)) {
            self.0.integrity.fail_closed();
        }

        result
    }

    /// Deletes Grants and audit events older than audit retention.
    pub async fn clean_up(&self, now: SystemTime) -> Result<(), StoreError> {
        self.mutate(move |inner| async move { inner.clean_up(now).await }).await
    }

    /// Spawned, so a caller that goes away cannot cut a mutation in half.
    async fn mutate<T, E, F>(&self, mutation: impl FnOnce(Arc<Inner>) -> F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        tokio::spawn(mutation(Arc::clone(&self.0))).await.map_err(|_| StoreError::Interrupted)?
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
        let deadline = Instant::now() + self.limits.grant_duration;
        self.active
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(address, Grant { expiry, deadline });

        Ok(expiry)
    }

    /// Renewal always passes; a new Address only below the cap, counting unexpired Grants.
    fn admit(&self, address: Address, now: SystemTime) -> Result<(), UnlockError> {
        let mut active = self.active.write().unwrap_or_else(PoisonError::into_inner);
        active.retain(|_, grant| grant.valid(now));
        let full = usize::try_from(self.limits.max_grants).is_ok_and(|max| active.len() >= max);

        if full && !active.contains_key(&address) {
            return Err(UnlockError::Full);
        }

        Ok(())
    }

    async fn revoke(&self, address: Address, now: SystemTime) -> Result<(), StoreError> {
        let mut connection = self.database.lock().await;

        self.active.write().unwrap_or_else(PoisonError::into_inner).remove(&address);
        let written = database::revoke(&mut connection, address, now).await;

        if written.is_err() {
            self.integrity.fail_closed();
        }

        written
    }

    async fn clean_up(&self, now: SystemTime) -> Result<(), StoreError> {
        let cutoff = now.checked_sub(self.limits.audit_retention).ok_or(StoreError::Time)?;
        let mut connection = self.database.lock().await;

        self.active.write().unwrap_or_else(PoisonError::into_inner).retain(|_, grant| grant.valid(now));
        database::delete_before(&mut connection, cutoff).await
    }
}
