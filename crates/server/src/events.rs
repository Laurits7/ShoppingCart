//! Live updates (SPEC §6.6, §7.3). After every write, the revs it produced tell which lists,
//! wishlists and claims changed and which users' own data or access changed; a subscriber is
//! nudged ("changed, rev N") only if it can see one of them at delivery time, or its own data
//! changed. The data itself always comes through `/sync`, which applies the same visibility
//! rules as every read (D-45).

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

/// What a write changed: the lists and wishlists touched, wishlists whose claims changed, and
/// users whose own data or access changed.
#[derive(Debug, Clone, Default)]
pub struct Change {
    pub rev: i64,
    pub lists: Vec<String>,
    pub wishlists: Vec<String>,
    pub claim_wishlists: Vec<String>,
    pub users: Vec<String>,
}

/// What a subscriber can see.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub lists: HashSet<String>,
    pub wishlists: HashSet<String>,
    /// Wishlists whose claims it sees (not its own).
    pub claims: HashSet<String>,
}

impl Change {
    /// Whether this may change what the subscriber sees, so its view must be looked up again.
    pub fn may_change_access(&self, me: &str, seen: &Seen) -> bool {
        self.users.iter().any(|u| u == me)
            || self.wishlists.iter().any(|w| seen.wishlists.contains(w))
    }

    /// Whether a subscriber should hear about it: `me`, who saw `before` when it happened and
    /// sees `now`. Lists: also if it could see them until now (so unsharing reaches it once).
    /// Wishlists and claims: only what it sees now, so a gift list's recipient or a wishlist's
    /// owner (for claims) never gets a hint (§7.3).
    pub fn concerns(&self, me: &str, before: &Seen, now: &Seen) -> bool {
        self.users.iter().any(|u| u == me)
            || self
                .lists
                .iter()
                .any(|l| before.lists.contains(l) || now.lists.contains(l))
            || self.wishlists.iter().any(|w| now.wishlists.contains(w))
            || self.claim_wishlists.iter().any(|w| now.claims.contains(w))
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
            users,
            ..Default::default()
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

async fn ids(pool: &SqlitePool, sql: &str, from: i64, to: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(sql)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await
}

/// What changed in revs `(from, to]`.
pub async fn changed(pool: &SqlitePool, from: i64, to: i64) -> sqlx::Result<Change> {
    let lists = ids(
        pool,
        "SELECT list_id FROM items WHERE rev > ?1 AND rev <= ?2
         UNION SELECT list_id FROM categories WHERE rev > ?1 AND rev <= ?2
         UNION SELECT id FROM lists WHERE rev > ?1 AND rev <= ?2
         UNION SELECT resource_id FROM shares WHERE resource_type = 'list' AND rev > ?1 AND rev <= ?2",
        from,
        to,
    )
    .await?;
    let wishlists = ids(
        pool,
        "SELECT wishlist_id FROM wish_items WHERE rev > ?1 AND rev <= ?2
         UNION SELECT wishlist_id FROM wish_sections WHERE rev > ?1 AND rev <= ?2
         UNION SELECT id FROM wishlists WHERE rev > ?1 AND rev <= ?2
         UNION SELECT resource_id FROM shares WHERE resource_type = 'wishlist' AND rev > ?1 AND rev <= ?2
         UNION SELECT w.id FROM wishlists w JOIN persons p ON p.id = w.recipient_person_id
               WHERE p.rev > ?1 AND p.rev <= ?2",
        from,
        to,
    )
    .await?;
    let claim_wishlists = ids(
        pool,
        "SELECT DISTINCT x.wishlist_id FROM claims c JOIN wish_items x ON x.id = c.wish_item_id
         WHERE c.rev > ?1 AND c.rev <= ?2",
        from,
        to,
    )
    .await?;
    // People reached by a changed share, never a gift list's recipient (no hints, §7.3).
    let not_recipient = |u: &str| {
        format!(
            "NOT (s.resource_type = 'wishlist' AND {u} IN (
                 SELECT p.user_id FROM wishlists w JOIN persons p ON p.id = w.recipient_person_id
                 WHERE w.id = s.resource_id AND p.user_id IS NOT NULL))"
        )
    };
    let users = ids(
        pool,
        &format!(
            "SELECT owner_id FROM shops WHERE rev > ?1 AND rev <= ?2
             UNION SELECT p.owner_id FROM staples s JOIN shops p ON p.id = s.shop_id WHERE s.rev > ?1 AND s.rev <= ?2
             UNION SELECT user_id FROM catalog_items WHERE rev > ?1 AND rev <= ?2
             UNION SELECT user_id FROM products WHERE rev > ?1 AND rev <= ?2
             UNION SELECT owner_id FROM persons WHERE rev > ?1 AND rev <= ?2
             UNION SELECT s.principal_id FROM shares s
                   WHERE s.principal_type = 'user' AND s.rev > ?1 AND s.rev <= ?2 AND {}
             UNION SELECT m.user_id FROM shares s JOIN household_members m ON m.household_id = s.principal_id
                   WHERE s.principal_type = 'household' AND s.rev > ?1 AND s.rev <= ?2 AND {}
             UNION SELECT m.user_id FROM households h JOIN household_members m ON m.household_id = h.id
                   WHERE h.rev > ?1 AND h.rev <= ?2",
            not_recipient("s.principal_id"),
            not_recipient("m.user_id"),
        ),
        from,
        to,
    )
    .await?;
    Ok(Change {
        rev: to,
        lists,
        wishlists,
        claim_wishlists,
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

    fn seen(lists: &[&str], wishlists: &[&str], claims: &[&str]) -> Seen {
        Seen {
            lists: set(lists),
            wishlists: set(wishlists),
            claims: set(claims),
        }
    }

    #[test]
    fn who_hears_about_lists() {
        let c = Change {
            rev: 5,
            lists: vec!["L1".into()],
            users: vec!["bob".into()],
            ..Default::default()
        };
        // Can see the list, or could until this very change (e.g. it was unshared).
        assert!(c.concerns("ann", &seen(&["L1"], &[], &[]), &Seen::default()));
        assert!(c.concerns("ann", &Seen::default(), &seen(&["L1"], &[], &[])));
        // Their own data or access changed.
        assert!(c.concerns("bob", &Seen::default(), &Seen::default()));
        // Nobody else.
        assert!(!c.concerns("eve", &seen(&["L2"], &[], &[]), &seen(&["L2"], &[], &[])));
    }

    #[test]
    fn wishlists_only_to_who_sees_them_now() {
        let c = Change {
            rev: 6,
            wishlists: vec!["W".into()],
            ..Default::default()
        };
        // Could see it, but not any more (e.g. just became its recipient): no nudge.
        assert!(!c.concerns("ann", &seen(&[], &["W"], &[]), &Seen::default()));
        assert!(c.may_change_access("ann", &seen(&[], &["W"], &[])));
        assert!(c.concerns("ann", &Seen::default(), &seen(&[], &["W"], &[])));
    }

    #[test]
    fn claims_never_reach_the_wishlist_owner() {
        let c = Change {
            rev: 7,
            claim_wishlists: vec!["W".into()],
            ..Default::default()
        };
        // The owner sees the wishlist, but not its claims.
        let owner = seen(&[], &["W"], &[]);
        assert!(!c.concerns("owner", &owner, &owner));
        let friend = seen(&[], &["W"], &["W"]);
        assert!(c.concerns("friend", &friend, &friend));
    }
}
