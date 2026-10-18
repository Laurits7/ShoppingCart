//! Live updates (SPEC §6.6, §7.3). After every write, the revs it produced tell which lists
//! and which users' personal data changed; a subscriber is nudged ("changed, rev N") only if
//! it can see one of those lists at delivery time, or its own data changed. The data itself
//! always comes through `/sync`, which applies the same visibility rules as every read.

use std::{collections::HashSet, sync::Arc};

use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::AppState;

/// What a write changed: revs `(from, to]`, the lists touched, and users whose own data or
/// access changed.
#[derive(Debug, Clone, Default)]
pub struct Change {
    pub rev: i64,
    pub lists: Vec<String>,
    pub users: Vec<String>,
}

impl Change {
    /// Whether a subscriber should hear about it: `me`, who could see `before` (the lists
    /// visible when the change happened) and can see `now` (after refreshing, if its access
    /// may have changed).
    pub fn concerns(&self, me: &str, before: &HashSet<String>, now: &HashSet<String>) -> bool {
        self.users.iter().any(|u| u == me)
            || self
                .lists
                .iter()
                .any(|l| before.contains(l) || now.contains(l))
    }
}

pub struct Bus {
    tx: broadcast::Sender<Arc<Change>>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            tx: broadcast::channel(1024).0,
        }
    }
}

impl Bus {
    pub fn publish(&self, c: Change) {
        // An error only means nobody is listening.
        let _ = self.tx.send(Arc::new(c));
    }

    /// Tell these users their access changed (e.g. they left a household).
    pub fn users(&self, rev: i64, users: Vec<String>) {
        self.publish(Change {
            rev,
            lists: vec![],
            users,
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Change>> {
        self.tx.subscribe()
    }
}

pub async fn current_rev(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT rev FROM meta_rev WHERE id = 1")
        .fetch_one(pool)
        .await
}

/// What changed in revs `(from, to]`.
pub async fn changed(pool: &SqlitePool, from: i64, to: i64) -> sqlx::Result<Change> {
    let lists: Vec<String> = sqlx::query_scalar(
        "SELECT list_id FROM items WHERE rev > ?1 AND rev <= ?2
         UNION SELECT list_id FROM categories WHERE rev > ?1 AND rev <= ?2
         UNION SELECT id FROM lists WHERE rev > ?1 AND rev <= ?2
         UNION SELECT resource_id FROM shares WHERE resource_type = 'list' AND rev > ?1 AND rev <= ?2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    let users: Vec<String> = sqlx::query_scalar(
        "SELECT owner_id FROM shops WHERE rev > ?1 AND rev <= ?2
         UNION SELECT p.owner_id FROM staples s JOIN shops p ON p.id = s.shop_id WHERE s.rev > ?1 AND s.rev <= ?2
         UNION SELECT user_id FROM catalog_items WHERE rev > ?1 AND rev <= ?2
         UNION SELECT user_id FROM products WHERE rev > ?1 AND rev <= ?2
         UNION SELECT principal_id FROM shares WHERE principal_type = 'user' AND rev > ?1 AND rev <= ?2
         UNION SELECT m.user_id FROM shares s JOIN household_members m ON m.household_id = s.principal_id
               WHERE s.principal_type = 'household' AND s.rev > ?1 AND s.rev <= ?2
         UNION SELECT m.user_id FROM households h JOIN household_members m ON m.household_id = h.id
               WHERE h.rev > ?1 AND h.rev <= ?2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(Change {
        rev: to,
        lists,
        users,
    })
}

/// Middleware: after a successful write, publish what it changed.
pub async fn publish_writes(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(req).await;
    }
    let before = current_rev(&state.db.write).await.ok();
    let res = next.run(req).await;
    if let Some(from) = before
        && res.status().is_success()
        && let Ok(to) = current_rev(&state.db.write).await
        && to > from
    {
        match changed(&state.db.write, from, to).await {
            Ok(c) => state.bus.publish(c),
            Err(e) => tracing::warn!("could not publish changes: {e}"),
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn who_hears_about_it() {
        let c = Change {
            rev: 5,
            lists: vec!["L1".into()],
            users: vec!["bob".into()],
        };
        // Can see the list, or could until this very change (e.g. it was unshared).
        assert!(c.concerns("ann", &set(&["L1"]), &set(&[])));
        assert!(c.concerns("ann", &set(&[]), &set(&["L1"])));
        // Their own data or access changed.
        assert!(c.concerns("bob", &set(&[]), &set(&[])));
        // Nobody else.
        assert!(!c.concerns("eve", &set(&["L2"]), &set(&["L2"])));
    }
}
