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

/// Ordering key after the last row of `table` whose `parent` column is `parent_id`
/// (tombstones included, so a restored row never shares a key).
pub async fn next_position(
    conn: &mut SqliteConnection,
    table: &str,
    parent: &str,
    parent_id: &str,
) -> sqlx::Result<String> {
    let last: Option<String> = sqlx::query_scalar(&format!(
        "SELECT MAX(position) FROM {table} WHERE {parent} = ?"
    ))
    .bind(parent_id)
    .fetch_one(conn)
    .await?;
    Ok(shoppingcart_domain::order::key_after(last.as_deref()))
}

/// Forget tombstones older than 30 days (children first, for the foreign keys) and
/// duplicate-guard records older than a day.
pub async fn purge(pool: &SqlitePool) -> sqlx::Result<()> {
    let ago = |days: i64| {
        (chrono::Utc::now() - chrono::Duration::days(days))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    };
    let old = ago(30);
    let mut tx = pool.begin().await?;
    for sql in [
        "DELETE FROM items WHERE deleted_at < ?1",
        "DELETE FROM categories WHERE deleted_at < ?1",
        "DELETE FROM staples WHERE deleted_at < ?1",
        "DELETE FROM product_preferences WHERE product_id IN (SELECT id FROM products WHERE deleted_at < ?1)",
        "UPDATE items SET product_id = NULL WHERE product_id IN (SELECT id FROM products WHERE deleted_at < ?1)",
        "UPDATE staples SET product_id = NULL WHERE product_id IN (SELECT id FROM products WHERE deleted_at < ?1)",
        "DELETE FROM products WHERE deleted_at < ?1",
        "DELETE FROM catalog_shop_uses WHERE catalog_item_id IN (SELECT id FROM catalog_items WHERE deleted_at < ?1)",
        "DELETE FROM catalog_items WHERE deleted_at < ?1",
        "DELETE FROM lists WHERE deleted_at < ?1 AND id NOT IN (SELECT list_id FROM items) AND id NOT IN (SELECT list_id FROM categories)",
        "DELETE FROM shops WHERE deleted_at < ?1 AND id NOT IN (SELECT shop_id FROM lists) AND id NOT IN (SELECT shop_id FROM staples)",
        "DELETE FROM claims WHERE deleted_at < ?1 OR wish_item_id IN (SELECT id FROM wish_items WHERE deleted_at < ?1)",
        "DELETE FROM wish_items WHERE deleted_at < ?1",
        "DELETE FROM wish_sections WHERE deleted_at < ?1",
        "DELETE FROM shares WHERE deleted_at < ?1",
        "DELETE FROM wishlists WHERE deleted_at < ?1 AND id NOT IN (SELECT wishlist_id FROM wish_items) AND id NOT IN (SELECT wishlist_id FROM wish_sections)",
        "DELETE FROM persons WHERE deleted_at < ?1 AND id NOT IN (SELECT recipient_person_id FROM wishlists WHERE recipient_person_id IS NOT NULL)",
    ] {
        sqlx::query(sql).bind(&old).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM item_merges WHERE created_at < ?")
        .bind(ago(1))
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}
