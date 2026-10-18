//! Catching up and live updates (SPEC §6.6): `/sync?since=<rev>` returns everything visible
//! that changed (and what was deleted); `/events` is a server-sent event stream that nudges a
//! client to sync when something it can see changes.

use std::{
    collections::HashSet,
    convert::Infallible,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{FromRequestParts, Query, State},
    http::request::Parts,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use tokio_stream::{
    StreamExt,
    wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
};
use ts_rs::TS;

use super::{catalog, items, products, shops, staples};
use crate::{
    AppState,
    auth::AuthUser,
    error::{ApiResult, AppError},
    events,
    models::{
        CatalogItem, Category, Image, Item, Product, ProductPreference, Shop, ShoppingList, Staple,
    },
    util::random_token,
    visibility::VISIBLE_LISTS,
};

/// Something deleted since the last sync.
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Deleted {
    #[ts(type = "'shop' | 'list' | 'category' | 'item' | 'staple' | 'catalog' | 'product'")]
    pub kind: String,
    pub id: String,
}

/// Everything visible that changed after `since` (with `since=0`: everything).
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ShoppingData {
    /// Pass this as `since` next time.
    #[ts(type = "number")]
    pub rev: i64,
    pub shops: Vec<Shop>,
    pub lists: Vec<ShoppingList>,
    pub categories: Vec<Category>,
    pub items: Vec<Item>,
    pub staples: Vec<Staple>,
    pub catalog: Vec<CatalogItem>,
    /// Your products, and others' products pinned on lists you see.
    pub products: Vec<Product>,
    /// Always all of them.
    pub preferences: Vec<ProductPreference>,
    /// Your pictures in use (for their credits); always all of them.
    pub images: Vec<Image>,
    pub deleted: Vec<Deleted>,
    /// Every list you can see now: drop any other you have (e.g. no longer shared with you).
    pub visible_lists: Vec<String>,
}

async fn deleted(conn: &mut SqliteConnection, uid: &str, since: i64) -> sqlx::Result<Vec<Deleted>> {
    // Only things that were yours or on lists you can see (or own); a list no longer shared
    // with you just drops out of `visible_lists`.
    let rows: Vec<(String, String)> = sqlx::query_as(&format!(
        "SELECT 'shop', id FROM shops WHERE owner_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2
         UNION ALL SELECT 'list', id FROM lists WHERE owner_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2
         UNION ALL SELECT 'category', c.id FROM categories c JOIN lists l ON l.id = c.list_id
               WHERE c.deleted_at IS NOT NULL AND c.rev > ?2 AND ({VISIBLE_LISTS} OR l.owner_id = ?1)
         UNION ALL SELECT 'item', i.id FROM items i JOIN lists l ON l.id = i.list_id
               WHERE i.deleted_at IS NOT NULL AND i.rev > ?2 AND ({VISIBLE_LISTS} OR l.owner_id = ?1)
         UNION ALL SELECT 'staple', s.id FROM staples s JOIN shops p ON p.id = s.shop_id
               WHERE p.owner_id = ?1 AND s.deleted_at IS NOT NULL AND s.rev > ?2
         UNION ALL SELECT 'catalog', id FROM catalog_items WHERE user_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2
         UNION ALL SELECT 'product', id FROM products WHERE user_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(kind, id)| Deleted { kind, id })
        .collect())
}

pub async fn visible_list_ids(conn: &mut SqliteConnection, uid: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(&format!("SELECT l.id FROM lists l WHERE {VISIBLE_LISTS}"))
        .bind(uid)
        .fetch_all(conn)
        .await
}

async fn load(state: &AppState, uid: &str, since: i64) -> ApiResult<ShoppingData> {
    let mut conn = state.db.read.begin().await?;
    // Read the rev first: anything written meanwhile comes again next time, never not at all.
    let rev: i64 = sqlx::query_scalar("SELECT rev FROM meta_rev WHERE id = 1")
        .fetch_one(&mut *conn)
        .await?;
    let images = sqlx::query_as(
        "SELECT id, width, height, source, source_ref, created_at FROM images WHERE owner_id = ?1
           AND (id IN (SELECT image_id FROM products WHERE user_id = ?1 AND deleted_at IS NULL)
                OR id IN (SELECT image_id FROM catalog_items WHERE user_id = ?1 AND deleted_at IS NULL))",
    )
    .bind(uid)
    .fetch_all(&mut *conn)
    .await?;
    Ok(ShoppingData {
        rev,
        shops: shops::user_shops(&mut conn, uid, since).await?,
        lists: shops::visible_lists(&mut conn, uid, None, since).await?,
        categories: items::visible_categories(&mut conn, uid, None, since).await?,
        items: items::visible_items(&mut conn, uid, None, since).await?,
        staples: staples::user_staples(&mut conn, uid, None, since).await?,
        catalog: catalog::since(&mut conn, uid, since).await?,
        products: products::synced_products(&mut conn, uid, since).await?,
        preferences: products::user_preferences(&mut conn, uid, None).await?,
        images,
        deleted: if since > 0 {
            deleted(&mut conn, uid, since).await?
        } else {
            vec![]
        },
        visible_lists: visible_list_ids(&mut conn, uid).await?,
    })
}

#[utoipa::path(get, path = "/shopping", tag = "shopping", summary = "Everything on your Shopping tab (the same as /sync?since=0)", responses((status = 200, body = ShoppingData), (status = 401, body = crate::error::Problem)))]
pub async fn shopping(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<ShoppingData>> {
    Ok(Json(load(&state, user.id(), 0).await?))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct Since {
    /// The `rev` of your last sync; 0 for everything.
    since: Option<i64>,
}

#[utoipa::path(get, path = "/sync", tag = "shopping", summary = "What changed since your last sync, including deletions", params(Since), responses((status = 200, body = ShoppingData), (status = 401, body = crate::error::Problem)))]
pub async fn sync(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<Since>,
) -> ApiResult<Json<ShoppingData>> {
    Ok(Json(
        load(&state, user.id(), q.since.unwrap_or(0).max(0)).await?,
    ))
}

// ---- live updates ------------------------------------------------------------------------

const TICKET_SECS: u64 = 60;

#[derive(Serialize, utoipa::ToSchema)]
pub struct Ticket {
    /// Use once, within a minute: `GET /events?ticket=…`.
    ticket: String,
}

#[utoipa::path(post, path = "/events/ticket", tag = "shopping", summary = "A one-time ticket for the event stream, for clients that can't send headers to it", responses((status = 200, body = Ticket)))]
pub async fn ticket(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Ticket>> {
    let t = random_token();
    let mut tickets = state.tickets.lock().await;
    tickets.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(TICKET_SECS));
    tickets.insert(t.clone(), (user.id().to_string(), Instant::now()));
    Ok(Json(Ticket { ticket: t }))
}

/// Who is listening: a signed-in browser (cookie), an API token, or a ticket.
pub struct Listener(String);

impl FromRequestParts<AppState> for Listener {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let ticket = parts.uri.query().and_then(|q| {
            q.split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == "ticket")
                .map(|(_, v)| v.to_string())
        });
        if let Some(t) = ticket {
            let (uid, at) = state
                .tickets
                .lock()
                .await
                .remove(&t)
                .ok_or(AppError::Unauthorized)?;
            if at.elapsed() > Duration::from_secs(TICKET_SECS) {
                return Err(AppError::Unauthorized);
            }
            return Ok(Listener(uid));
        }
        let user = AuthUser::from_request_parts(parts, state).await?;
        Ok(Listener(user.id().to_string()))
    }
}

#[utoipa::path(get, path = "/events", tag = "shopping", summary = "Server-sent events: `change` with the new rev when something you can see changed (then call /sync), `resync` when you should sync anyway", params(("ticket" = Option<String>, Query, description = "From POST /events/ticket, for clients that can't send the cookie or Authorization header")), responses((status = 200, description = "text/event-stream")))]
pub async fn events(
    State(state): State<AppState>,
    Listener(me): Listener,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let rx = state.bus.subscribe();
    let visible: HashSet<String> = {
        let mut conn = state.db.read.acquire().await?;
        visible_list_ids(&mut conn, &me)
            .await?
            .into_iter()
            .collect()
    };
    let rev = events::current_rev(&state.db.read).await?;
    let hello = stream::once(async move {
        Ok::<_, Infallible>(
            Event::default()
                .event("hello")
                .data(format!("{{\"rev\":{rev}}}")),
        )
    });
    // Each change: look again at what this subscriber can see if its access may have changed,
    // and nudge it only if the change concerns it.
    let changes = stream::unfold(
        (BroadcastStream::new(rx), state, me, visible),
        |(mut rx, state, me, mut visible)| async move {
            loop {
                match rx.next().await? {
                    Ok(c) => {
                        let now = if c.users.contains(&me) {
                            match state.db.read.acquire().await {
                                Ok(mut conn) => visible_list_ids(&mut conn, &me)
                                    .await
                                    .map(|v| v.into_iter().collect())
                                    .unwrap_or_else(|_| visible.clone()),
                                Err(_) => visible.clone(),
                            }
                        } else {
                            visible.clone()
                        };
                        let send = c.concerns(&me, &visible, &now);
                        visible = now;
                        if send {
                            let e = Event::default()
                                .event("change")
                                .data(format!("{{\"rev\":{}}}", c.rev));
                            return Some((Ok(e), (rx, state, me, visible)));
                        }
                    }
                    // Missed some: the client syncs anyway.
                    Err(BroadcastStreamRecvError::Lagged(_)) => {
                        let e = Event::default().event("resync").data("{}");
                        return Some((Ok(e), (rx, state, me, visible)));
                    }
                }
            }
        },
    );
    Ok(Sse::new(hello.chain(changes))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(25))))
}
