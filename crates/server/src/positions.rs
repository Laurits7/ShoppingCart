//! Placing a row among its siblings (SPEC §6.5): a move names the sibling to go after or
//! before, and the server computes the fractional-index key, so a move writes one row and
//! concurrent moves never corrupt the order. Scopes whose keys grow long are rebalanced.

use serde::Deserialize;
use shoppingcart_domain::order::{self, MAX_KEY_LEN};
use sqlx::SqliteConnection;

use crate::{
    db::next_rev,
    error::{ApiResult, bad},
    util::now,
};

/// Where a moved row goes among its siblings: right after one, right before one, or (with
/// neither) at the end.
#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct Place {
    pub after: Option<String>,
    pub before: Option<String>,
}

/// A set of siblings: rows of `table` whose `column` is `id`.
pub struct Scope<'a> {
    pub table: &'static str,
    pub column: &'static str,
    pub id: &'a str,
}

impl Scope<'_> {
    async fn sibling(&self, conn: &mut SqliteConnection, id: &str) -> ApiResult<String> {
        sqlx::query_scalar(&format!(
            "SELECT position FROM {} WHERE id = ? AND {} = ? AND deleted_at IS NULL",
            self.table, self.column
        ))
        .bind(id)
        .bind(self.id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| bad("after/before must name a sibling in the same place"))
    }

    /// The nearest live sibling (other than `moving`) after or before `(position, id)`.
    async fn next(
        &self,
        conn: &mut SqliteConnection,
        moving: &str,
        position: &str,
        id: &str,
        forward: bool,
    ) -> sqlx::Result<Option<String>> {
        let (cmp, dir) = if forward { (">", "") } else { ("<", " DESC") };
        sqlx::query_scalar(&format!(
            "SELECT position FROM {t} WHERE {c} = ?1 AND deleted_at IS NULL AND id != ?2
               AND (position, id) {cmp} (?3, ?4)
             ORDER BY position{dir}, id{dir} LIMIT 1",
            t = self.table,
            c = self.column
        ))
        .bind(self.id)
        .bind(moving)
        .bind(position)
        .bind(id)
        .fetch_optional(conn)
        .await
    }

    async fn last(
        &self,
        conn: &mut SqliteConnection,
        moving: &str,
    ) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar(&format!(
            "SELECT MAX(position) FROM {} WHERE {} = ? AND id != ?",
            self.table, self.column
        ))
        .bind(self.id)
        .bind(moving)
        .fetch_one(conn)
        .await
    }

    /// Give every live sibling an evenly spaced short key, keeping their order.
    pub async fn rebalance(&self, conn: &mut SqliteConnection) -> sqlx::Result<()> {
        let ids: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT id FROM {} WHERE {} = ? AND deleted_at IS NULL ORDER BY position, id",
            self.table, self.column
        ))
        .bind(self.id)
        .fetch_all(&mut *conn)
        .await?;
        let rev = next_rev(conn).await?;
        let ts = now();
        for (id, key) in ids.iter().zip(order::spread(ids.len())) {
            sqlx::query(&format!(
                "UPDATE {} SET position = ?, updated_at = ?, rev = ? WHERE id = ?",
                self.table
            ))
            .bind(key)
            .bind(&ts)
            .bind(rev)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }

    /// The key for `moving` at `place`, rebalancing the scope first if needed.
    pub async fn key_for(
        &self,
        conn: &mut SqliteConnection,
        moving: &str,
        place: &Place,
    ) -> ApiResult<String> {
        for attempt in 0..2 {
            let (lo, hi) = match (&place.after, &place.before) {
                (Some(a), _) => {
                    let p = self.sibling(conn, a).await?;
                    let hi = self.next(conn, moving, &p, a, true).await?;
                    (Some(p), hi)
                }
                (None, Some(b)) => {
                    let p = self.sibling(conn, b).await?;
                    let lo = self.next(conn, moving, &p, b, false).await?;
                    (lo, Some(p))
                }
                (None, None) => (self.last(conn, moving).await?, None),
            };
            // Equal neighbouring keys (or very long ones) need room first.
            let squeezed = matches!((&lo, &hi), (Some(l), Some(h)) if l >= h);
            if !squeezed {
                let key = order::key_between(lo.as_deref().unwrap_or(""), hi.as_deref());
                if key.len() <= MAX_KEY_LEN || attempt == 1 {
                    return Ok(key);
                }
            }
            self.rebalance(conn).await?;
        }
        unreachable!("the second attempt always returns")
    }
}
