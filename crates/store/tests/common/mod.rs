//! Shared fixtures: a store in a temporary directory and a fixed wall clock.

#![allow(dead_code, reason = "each test binary uses a different subset")]
#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use access::Address;
use store::{Grants, Limits};
use tempfile::TempDir;

pub const HOUR: Duration = Duration::from_hours(1);
pub const DAY: Duration = Duration::from_hours(24);
pub const WEEK: Duration = Duration::from_hours(7 * 24);

/// 2027-01-15T08:00:00Z.
pub fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_hours(500_000)
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// The spec's defaults (section 6 and 7): 7-day Grants, 1,000 at most, 7-day audit retention.
pub fn limits() -> Limits {
    Limits { grant_duration: WEEK, max_grants: 1_000, audit_retention: WEEK }
}

#[derive(Debug)]
pub struct Database {
    _dir: TempDir,
    pub path: PathBuf,
}

impl Database {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accit.db");

        Self { _dir: dir, path }
    }

    pub async fn open(&self, limits: Limits, now: SystemTime) -> Grants {
        Grants::open(Some(&self.path), limits, now).await.unwrap()
    }
}

impl Database {
    /// Makes every `operation` (`INSERT`, `DELETE`) on `table` fail, as a full or broken disk would.
    pub async fn fail(&self, operation: &str, table: &str) {
        self.execute(format!(
            "CREATE TRIGGER fail_{operation}_{table} BEFORE {operation} ON {table}
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
        ))
        .await;
    }

    /// Undoes [`Self::fail`].
    pub async fn heal(&self, operation: &str, table: &str) {
        self.execute(format!("DROP TRIGGER fail_{operation}_{table}")).await;
    }

    async fn execute(&self, sql: String) {
        use sqlx::{Connection, SqliteConnection};

        let mut connection =
            SqliteConnection::connect(&format!("sqlite://{}", self.path.display())).await.unwrap();

        sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(&mut connection).await.unwrap();
    }
}
