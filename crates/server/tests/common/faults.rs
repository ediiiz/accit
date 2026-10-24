//! Faults injected into a Harness's database, as a broken disk would cause them.

#![allow(clippy::unwrap_used, reason = "test helpers: a broken fixture is a test bug")]

use sqlx::{Connection, SqliteConnection};

use super::harness::Harness;

impl Harness {
    /// Every operation on `table` (`INSERT`, `DELETE`) fails, as a broken disk would.
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
        let mut connection =
            SqliteConnection::connect(&format!("sqlite://{}", self.database.display())).await.unwrap();

        sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(&mut connection).await.unwrap();
    }
}
