use std::{path::Path, str::FromStr, time::Duration};

use sqlx::{
    SqliteConnection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

/// SQLite with a single writer connection (so write transactions never fight over the
/// lock) and a small pool of readers (WAL lets them run concurrently).
#[derive(Clone)]
pub struct Db {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

impl Db {
    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone())
            .await?;
        sqlx::migrate!("../../migrations").run(&write).await?;
        let read = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts.read_only(true))
            .await?;
        Ok(Self { read, write })
    }
}

/// Allocate the next global change number. Must be called inside a write transaction.
pub async fn next_rev(conn: &mut SqliteConnection) -> sqlx::Result<i64> {
    sqlx::query_scalar("UPDATE meta_rev SET rev = rev + 1 WHERE id = 1 RETURNING rev")
        .fetch_one(conn)
        .await
}
