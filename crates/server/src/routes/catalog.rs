//! The item catalogue (SPEC §6.4a, D-10, D-20): everything a user has added, per user,
//! recorded on every add; suggestions, frequent items and its management.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use shoppingcart_domain::{
    amount,
    catalog::{self, Usage},
    shopping,
};
use sqlx::SqliteConnection;

use crate::{
    AppState,
    auth::AuthUser,
    db::next_rev,
    error::{ApiResult, AppError, bad},
    models::{CatalogItem, CatalogShopUse},
    util::{double_option, name, new_id, now},
    visibility::{self, Need},
};

const COLS: &str = "id, name, category_name, amount, unit, step, use_count, last_used_at, shop_id, hidden, image_id, created_at, updated_at, rev";

/// What the catalogue remembers about a name, used to fill in a new item.
#[derive(sqlx::FromRow)]
pub struct Known {
    pub id: String,
    pub category_name: Option<String>,
    pub amount: f64,
    pub unit: Option<String>,
    pub step: Option<f64>,
}

/// The user's live entry for a (normalised) name.
pub async fn lookup(
    conn: &mut SqliteConnection,
    user_id: &str,
    normalized: &str,
) -> sqlx::Result<Option<Known>> {
    sqlx::query_as(
        "SELECT id, category_name, amount, unit, step FROM catalog_items
         WHERE user_id = ? AND normalized_name = ? AND deleted_at IS NULL",
    )
    .bind(user_id)
    .bind(normalized)
    .fetch_optional(conn)
    .await
}

/// One use of an item, as it was added.
pub struct Use<'a> {
    pub name: &'a str,
    pub category_name: Option<&'a str>,
    pub amount: f64,
    pub unit: Option<&'a str>,
    pub step: Option<f64>,
    pub shop_id: Option<&'a str>,
}

/// Record that the user added `u`: create the entry or count the use, remember the category
/// (when there is one), amount, unit and shop. Returns the entry's id.
pub async fn record(
    conn: &mut SqliteConnection,
    user_id: &str,
    u: &Use<'_>,
) -> sqlx::Result<String> {
    let normalized = catalog::normalize_name(u.name);
    let rev = next_rev(conn).await?;
    let ts = now();
    let id: String = sqlx::query_scalar(
        "INSERT INTO catalog_items (id, user_id, name, normalized_name, category_name, amount, unit, step,
                                    use_count, last_used_at, shop_id, created_at, updated_at, rev)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?9, ?9, ?11)
         ON CONFLICT (user_id, normalized_name) WHERE deleted_at IS NULL DO UPDATE SET
             category_name = COALESCE(excluded.category_name, category_name),
             amount = excluded.amount, unit = excluded.unit,
             step = COALESCE(excluded.step, step),
             use_count = use_count + 1, last_used_at = excluded.last_used_at,
             shop_id = COALESCE(excluded.shop_id, shop_id),
             updated_at = excluded.updated_at, rev = excluded.rev
         RETURNING id",
    )
    .bind(new_id())
    .bind(user_id)
    .bind(u.name)
    .bind(&normalized)
    .bind(u.category_name)
    .bind(u.amount)
    .bind(u.unit)
    .bind(u.step)
    .bind(&ts)
    .bind(u.shop_id)
    .bind(rev)
    .fetch_one(&mut *conn)
    .await?;
    if let Some(shop) = u.shop_id {
        sqlx::query(
            "INSERT INTO catalog_shop_uses (catalog_item_id, shop_id, use_count, last_used_at) VALUES (?,?,1,?)
             ON CONFLICT (catalog_item_id, shop_id) DO UPDATE SET
                 use_count = use_count + 1, last_used_at = excluded.last_used_at",
        )
        .bind(&id)
        .bind(shop)
        .bind(&ts)
        .execute(&mut *conn)
        .await?;
    }
    Ok(id)
}

/// Remember the category an item was moved into, so it lands there next time.
pub async fn learn_category(
    conn: &mut SqliteConnection,
    user_id: &str,
    catalog_item_id: &str,
    category_name: Option<&str>,
) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query(
        "UPDATE catalog_items SET category_name = ?1, updated_at = ?2, rev = ?3
         WHERE id = ?4 AND user_id = ?5 AND deleted_at IS NULL AND category_name IS NOT ?1",
    )
    .bind(category_name)
    .bind(now())
    .bind(rev)
    .bind(catalog_item_id)
    .bind(user_id)
    .execute(conn)
    .await?;
    Ok(())
}

async fn with_shops(
    conn: &mut SqliteConnection,
    user_id: &str,
    mut items: Vec<CatalogItem>,
) -> sqlx::Result<Vec<CatalogItem>> {
    let uses: Vec<(String, String, i64, String)> = sqlx::query_as(
        "SELECT u.catalog_item_id, u.shop_id, u.use_count, u.last_used_at FROM catalog_shop_uses u
         JOIN catalog_items c ON c.id = u.catalog_item_id
         WHERE c.user_id = ? AND c.deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await?;
    let mut by_item: std::collections::HashMap<String, Vec<CatalogShopUse>> = Default::default();
    for (item, shop_id, use_count, last_used_at) in uses {
        by_item.entry(item).or_default().push(CatalogShopUse {
            shop_id,
            use_count,
            last_used_at,
        });
    }
    for i in &mut items {
        i.shops = by_item.remove(&i.id).unwrap_or_default();
    }
    Ok(items)
}

/// The user's whole catalogue (hidden entries too), by name.
pub async fn all(conn: &mut SqliteConnection, user_id: &str) -> sqlx::Result<Vec<CatalogItem>> {
    let items = sqlx::query_as(&format!(
        "SELECT {COLS} FROM catalog_items WHERE user_id = ? AND deleted_at IS NULL ORDER BY normalized_name"
    ))
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?;
    with_shops(conn, user_id, items).await
}

async fn load(conn: &mut SqliteConnection, user_id: &str, id: &str) -> ApiResult<CatalogItem> {
    all(conn, user_id)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or(AppError::NotFound)
}

fn days_since(ts: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> f64 {
    ts.and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| (now - t.to_utc()).num_seconds() as f64 / 86400.0)
        .unwrap_or(365.0)
}

fn usage(c: &CatalogItem, shop_id: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> Usage {
    Usage {
        use_count: c.use_count,
        days_since_use: days_since(c.last_used_at.as_deref(), now),
        shop_count: shop_id
            .and_then(|s| c.shops.iter().find(|u| u.shop_id == s))
            .map_or(0, |u| u.use_count),
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CatalogQuery {
    /// What's been typed so far. Without it, the whole catalogue (hidden entries too).
    q: Option<String>,
    /// Boost what's usually bought at this shop.
    shop_id: Option<String>,
    /// At most this many suggestions (default 10, max 100).
    limit: Option<usize>,
}

#[utoipa::path(get, path = "/catalog", tag = "catalog", summary = "Suggestions from your item catalogue (or all of it)", params(CatalogQuery), responses((status = 200, body = Vec<CatalogItem>), (status = 401, body = crate::error::Problem)))]
pub async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<CatalogQuery>,
) -> ApiResult<Json<Vec<CatalogItem>>> {
    let mut conn = state.db.read.acquire().await?;
    let entries = all(&mut conn, user.id()).await?;
    let Some(query) = q.q.filter(|s| !s.trim().is_empty()) else {
        return Ok(Json(entries));
    };
    let visible: Vec<CatalogItem> = entries.into_iter().filter(|c| !c.hidden).collect();
    let t = chrono::Utc::now();
    let names: Vec<String> = visible
        .iter()
        .map(|c| catalog::normalize_name(&c.name))
        .collect();
    let ranked: Vec<(&str, Usage)> = visible
        .iter()
        .zip(&names)
        .map(|(c, n)| (n.as_str(), usage(c, q.shop_id.as_deref(), t)))
        .collect();
    let order = catalog::rank(&ranked, &query, q.limit.unwrap_or(10).min(100));
    Ok(Json(
        order.into_iter().map(|i| visible[i].clone()).collect(),
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct FrequentQuery {
    /// The list being added to: its shop's favourites that aren't open on it.
    list_id: String,
    /// At most this many (default 12, max 50).
    limit: Option<usize>,
}

#[utoipa::path(get, path = "/catalog/frequent", tag = "catalog", summary = "What you buy most at the list's shop and isn't on the list", params(FrequentQuery), responses((status = 200, body = Vec<CatalogItem>), (status = 404, body = crate::error::Problem)))]
pub async fn frequent(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<FrequentQuery>,
) -> ApiResult<Json<Vec<CatalogItem>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_list(&mut conn, &user, &q.list_id, Need::Read).await?;
    let shop_id: String = sqlx::query_scalar("SELECT shop_id FROM lists WHERE id = ?")
        .bind(&q.list_id)
        .fetch_one(&mut *conn)
        .await?;
    let open: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM items WHERE list_id = ? AND deleted_at IS NULL AND checked_at IS NULL",
    )
    .bind(&q.list_id)
    .fetch_all(&mut *conn)
    .await?;
    let open: std::collections::HashSet<String> =
        open.iter().map(|n| catalog::normalize_name(n)).collect();
    let t = chrono::Utc::now();
    let candidates: Vec<CatalogItem> = all(&mut conn, user.id())
        .await?
        .into_iter()
        .filter(|c| !c.hidden && !open.contains(&catalog::normalize_name(&c.name)))
        .collect();
    let names: Vec<String> = candidates
        .iter()
        .map(|c| catalog::normalize_name(&c.name))
        .collect();
    let ranked: Vec<(&str, Usage)> = candidates
        .iter()
        .zip(&names)
        .map(|(c, n)| (n.as_str(), usage(c, Some(&shop_id), t)))
        .collect();
    let order = catalog::frequent(&ranked, q.limit.unwrap_or(12).min(50));
    Ok(Json(
        order.into_iter().map(|i| candidates[i].clone()).collect(),
    ))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchCatalogItem {
    name: Option<String>,
    /// The category it goes into; null for none.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    category_name: Option<Option<String>>,
    amount: Option<f64>,
    /// Null for pieces.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    unit: Option<Option<String>>,
    /// Null for the unit's usual step.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<f64>)]
    step: Option<Option<f64>>,
    hidden: Option<bool>,
    /// A picture you uploaded, or null for none.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    image_id: Option<Option<String>>,
}

#[utoipa::path(patch, path = "/catalog/{id}", tag = "catalog", summary = "Rename an entry, set its category, amount, unit or step, or hide it", params(("id" = String, Path, description = "ULID")), request_body = PatchCatalogItem, responses((status = 200, body = CatalogItem), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, description = "Another entry has that name: merge them", body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchCatalogItem>,
) -> ApiResult<Json<CatalogItem>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_catalog_item(&mut tx, &user, &id).await?;
    if let Some(n) = p.name {
        let n = name("name", &n, shopping::MAX_NAME_CHARS)?;
        let normalized = catalog::normalize_name(&n);
        if lookup(&mut tx, user.id(), &normalized)
            .await?
            .is_some_and(|k| k.id != id)
        {
            return Err(AppError::Conflict(
                "another entry already has this name; merge them instead".into(),
            ));
        }
        sqlx::query("UPDATE catalog_items SET name = ?, normalized_name = ? WHERE id = ?")
            .bind(&n)
            .bind(&normalized)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(c) = p.category_name {
        let c = c
            .filter(|c| !c.trim().is_empty())
            .map(|c| name("category", &c, shopping::MAX_NAME_CHARS))
            .transpose()?;
        sqlx::query("UPDATE catalog_items SET category_name = ? WHERE id = ?")
            .bind(c)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(a) = p.amount {
        sqlx::query("UPDATE catalog_items SET amount = ? WHERE id = ?")
            .bind(amount::check_amount(a).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(u) = p.unit {
        sqlx::query("UPDATE catalog_items SET unit = ? WHERE id = ?")
            .bind(amount::check_unit(u.as_deref().unwrap_or("")).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(s) = p.step {
        sqlx::query("UPDATE catalog_items SET step = ? WHERE id = ?")
            .bind(s.map(amount::check_amount).transpose().map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(img) = p.image_id {
        if let Some(i) = &img {
            visibility::require_own_image(&mut tx, &user, i).await?;
        }
        sqlx::query("UPDATE catalog_items SET image_id = ? WHERE id = ?")
            .bind(img)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(h) = p.hidden {
        sqlx::query("UPDATE catalog_items SET hidden = ? WHERE id = ?")
            .bind(h)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE catalog_items SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let item = load(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(item))
}

/// Forget catalogue entries and their products; items and staples using them stay,
/// unlinked.
async fn tombstone(conn: &mut SqliteConnection, ids: &[String]) -> sqlx::Result<()> {
    for id in ids {
        super::products::tombstone(conn, "catalog_item_id = ?3", id).await?;
        sqlx::query("DELETE FROM product_preferences WHERE catalog_item_id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    let rev = next_rev(conn).await?;
    let ts = now();
    for id in ids {
        sqlx::query(
            "UPDATE catalog_items SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3",
        )
        .bind(&ts)
        .bind(rev)
        .bind(id)
        .execute(&mut *conn)
        .await?;
        for table in ["items", "staples"] {
            sqlx::query(&format!(
                "UPDATE {table} SET catalog_item_id = NULL, updated_at = ?1, rev = ?2 WHERE catalog_item_id = ?3"
            ))
            .bind(&ts)
            .bind(rev)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        }
        sqlx::query("DELETE FROM catalog_shop_uses WHERE catalog_item_id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

#[utoipa::path(delete, path = "/catalog/{id}", tag = "catalog", summary = "Forget a catalogue entry (items on lists stay)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_catalog_item(&mut tx, &user, &id).await?;
    tombstone(&mut tx, &[id]).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MergeCatalog {
    /// The entry to keep.
    into: String,
    /// Entries folded into it and then forgotten.
    from: Vec<String>,
}

#[utoipa::path(post, path = "/catalog/merge", tag = "catalog", summary = "Merge duplicate entries (\"Tomato\" + \"Tomatoes\")", request_body = MergeCatalog, responses((status = 200, body = CatalogItem), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn merge(
    State(state): State<AppState>,
    user: AuthUser,
    Json(m): Json<MergeCatalog>,
) -> ApiResult<Json<CatalogItem>> {
    let mut from = m.from;
    from.sort();
    from.dedup();
    from.retain(|f| *f != m.into);
    if from.is_empty() {
        return Err(bad("nothing to merge"));
    }
    if from.len() > 100 {
        return Err(bad("merge at most 100 entries at once"));
    }
    let mut tx = state.db.write.begin().await?;
    visibility::require_catalog_item(&mut tx, &user, &m.into).await?;
    for f in &from {
        visibility::require_catalog_item(&mut tx, &user, f).await?;
    }
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    for f in &from {
        // Counts add up; the latest use and a missing category carry over.
        sqlx::query(
            "UPDATE catalog_items SET
                 use_count = use_count + (SELECT use_count FROM catalog_items WHERE id = ?2),
                 last_used_at = MAX(COALESCE(last_used_at, ''), COALESCE((SELECT last_used_at FROM catalog_items WHERE id = ?2), '')),
                 category_name = COALESCE(category_name, (SELECT category_name FROM catalog_items WHERE id = ?2))
             WHERE id = ?1",
        )
        .bind(&m.into)
        .bind(f)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO catalog_shop_uses (catalog_item_id, shop_id, use_count, last_used_at)
             SELECT ?1, shop_id, use_count, last_used_at FROM catalog_shop_uses WHERE catalog_item_id = ?2
             ON CONFLICT (catalog_item_id, shop_id) DO UPDATE SET
                 use_count = use_count + excluded.use_count,
                 last_used_at = MAX(last_used_at, excluded.last_used_at)",
        )
        .bind(&m.into)
        .bind(f)
        .execute(&mut *tx)
        .await?;
        // Its products and preferences come along (the kept entry's preferences win).
        sqlx::query(
            "UPDATE products SET catalog_item_id = ?1, updated_at = ?2, rev = ?3 WHERE catalog_item_id = ?4",
        )
        .bind(&m.into)
        .bind(&ts)
        .bind(rev)
        .bind(f)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO product_preferences (catalog_item_id, shop_id, product_id)
             SELECT ?1, shop_id, product_id FROM product_preferences WHERE catalog_item_id = ?2",
        )
        .bind(&m.into)
        .bind(f)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE catalog_items SET image_id = COALESCE(image_id, (SELECT image_id FROM catalog_items WHERE id = ?2)) WHERE id = ?1",
        )
        .bind(&m.into)
        .bind(f)
        .execute(&mut *tx)
        .await?;
        for table in ["items", "staples"] {
            sqlx::query(&format!(
                "UPDATE {table} SET catalog_item_id = ?1, updated_at = ?2, rev = ?3 WHERE catalog_item_id = ?4"
            ))
            .bind(&m.into)
            .bind(&ts)
            .bind(rev)
            .bind(f)
            .execute(&mut *tx)
            .await?;
        }
    }
    sqlx::query(
        "UPDATE catalog_items SET last_used_at = NULLIF(last_used_at, ''), updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&ts)
    .bind(rev)
    .bind(&m.into)
    .execute(&mut *tx)
    .await?;
    tombstone(&mut tx, &from).await?;
    let item = load(&mut tx, user.id(), &m.into).await?;
    tx.commit().await?;
    Ok(Json(item))
}
