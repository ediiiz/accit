//! The database side. Every function runs inside the caller's hold on the mutation lock.

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use access::Address;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{ConnectOptions, Connection, SqliteConnection};

use crate::StoreError;
use crate::audit::{Action, AuditEvent};
use crate::time::{from_millis, to_millis};

pub(crate) async fn connect(path: Option<&Path>) -> Result<SqliteConnection, StoreError> {
    let options = match path {
        Some(path) => SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full),
        None => SqliteConnectOptions::new().in_memory(true),
    };
    let mut connection = options.connect().await?;

    sqlx::migrate!().run(&mut connection).await?;

    Ok(connection)
}

pub(crate) async fn unexpired(
    connection: &mut SqliteConnection,
    now: SystemTime,
) -> Result<HashMap<Address, SystemTime>, StoreError> {
    let now = to_millis(now)?;
    let rows = sqlx::query!("SELECT address, expires_at FROM grants WHERE expires_at > ?", now)
        .fetch_all(connection)
        .await?;

    rows.into_iter().map(|row| Ok((row.address.parse()?, from_millis(row.expires_at)?))).collect()
}

pub(crate) async fn unlock(
    connection: &mut SqliteConnection,
    address: Address,
    expiry: SystemTime,
    now: SystemTime,
) -> Result<(), StoreError> {
    let (text, expires_at) = (address.to_string(), to_millis(expiry)?);
    let mut transaction = connection.begin().await?;

    sqlx::query!(
        "INSERT INTO grants (address, expires_at) VALUES (?, ?)
         ON CONFLICT (address) DO UPDATE SET expires_at = excluded.expires_at",
        text,
        expires_at,
    )
    .execute(&mut *transaction)
    .await?;
    record(&mut transaction, Action::Unlock, address, now).await?;
    transaction.commit().await?;

    Ok(())
}

pub(crate) async fn revoke(
    connection: &mut SqliteConnection,
    address: Address,
    now: SystemTime,
) -> Result<(), StoreError> {
    let text = address.to_string();
    let mut transaction = connection.begin().await?;

    sqlx::query!("DELETE FROM grants WHERE address = ?", text).execute(&mut *transaction).await?;
    record(&mut transaction, Action::Revoke, address, now).await?;
    transaction.commit().await?;

    Ok(())
}

/// Unexpired Grants, each with its Revoke audit event; returns how many.
pub(crate) async fn revoke_all(
    connection: &mut SqliteConnection,
    now: SystemTime,
) -> Result<u64, StoreError> {
    let (action, now) = (Action::Revoke.as_str(), to_millis(now)?);
    let mut transaction = connection.begin().await?;

    sqlx::query!(
        "INSERT INTO audit_events (action, address, at) SELECT ?, address, ? FROM grants WHERE expires_at > ?",
        action,
        now,
        now,
    )
    .execute(&mut *transaction)
    .await?;
    let deleted =
        sqlx::query!("DELETE FROM grants WHERE expires_at > ?", now).execute(&mut *transaction).await?;
    transaction.commit().await?;

    Ok(deleted.rows_affected())
}

async fn record(
    connection: &mut SqliteConnection,
    action: Action,
    address: Address,
    at: SystemTime,
) -> Result<(), StoreError> {
    let (action, address, at) = (action.as_str(), address.to_string(), to_millis(at)?);

    sqlx::query!("INSERT INTO audit_events (action, address, at) VALUES (?, ?, ?)", action, address, at)
        .execute(connection)
        .await?;

    Ok(())
}

pub(crate) async fn audit_events(connection: &mut SqliteConnection) -> Result<Vec<AuditEvent>, StoreError> {
    let rows = sqlx::query!("SELECT action, address, at FROM audit_events ORDER BY id")
        .fetch_all(connection)
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(AuditEvent {
                action: Action::parse(&row.action)?,
                address: row.address.parse()?,
                at: from_millis(row.at)?,
            })
        })
        .collect()
}

pub(crate) async fn expiry(
    connection: &mut SqliteConnection,
    address: Address,
) -> Result<Option<SystemTime>, StoreError> {
    let address = address.to_string();
    let stored = sqlx::query_scalar!("SELECT expires_at FROM grants WHERE address = ?", address)
        .fetch_optional(connection)
        .await?;

    stored.map(from_millis).transpose()
}

/// Grants that expired, and audit events that happened, at or before `cutoff`.
pub(crate) async fn delete_before(
    connection: &mut SqliteConnection,
    cutoff: SystemTime,
) -> Result<(), StoreError> {
    let cutoff = to_millis(cutoff)?;
    let mut transaction = connection.begin().await?;

    sqlx::query!("DELETE FROM grants WHERE expires_at <= ?", cutoff).execute(&mut *transaction).await?;
    sqlx::query!("DELETE FROM audit_events WHERE at <= ?", cutoff).execute(&mut *transaction).await?;
    transaction.commit().await?;

    Ok(())
}
