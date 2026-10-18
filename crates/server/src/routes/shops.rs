//! Shops, lists, templates and the inbox (SPEC §6.1–6.3, D-65). A shop has shopping lists
//! and templates (usual items to start lists from); a new list can be a copy of a template.
//! Items that come without a list (Mealie, the API) land in the user's inbox.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use shoppingcart_domain::{order, shopping};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::items;
use crate::{
    AppState,
    auth::AuthUser,
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::{Category, Shop, ShoppingList},
    positions::{Place, Scope},
    util::{double_option, id_or_new, name, now},
    visibility::{self, Need, VISIBLE_LISTS},
};

const SHOP_COLS: &str = "id, name, color, position, created_at, updated_at, rev";

pub async fn load_shop(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Option<Shop>> {
    sqlx::query_as(&format!(
        "SELECT {SHOP_COLS} FROM shops WHERE id = ? AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

/// The user's live shops changed after rev `since` (0 = all).
pub async fn user_shops(
    conn: &mut SqliteConnection,
    user_id: &str,
    since: i64,
) -> sqlx::Result<Vec<Shop>> {
    sqlx::query_as(&format!(
        "SELECT {SHOP_COLS} FROM shops WHERE owner_id = ? AND deleted_at IS NULL AND rev > ? ORDER BY position, id"
    ))
    .bind(user_id)
    .bind(since)
    .fetch_all(conn)
    .await
}

/// Lists visible to the user bound as `?1`, narrowed by `cond`.
fn list_select(cond: &str) -> String {
    let role = visibility::role_sql();
    format!(
        "SELECT l.id, l.shop_id, l.owner_id, l.kind, l.name, l.position, l.archived_at, {role} AS role,
                COALESCE(s.name, '') AS shop_name, u.display_name AS owner_name,
                (SELECT COUNT(*) FROM items i
                 WHERE i.list_id = l.id AND i.deleted_at IS NULL AND i.checked_at IS NULL) AS open_count,
                l.created_at, l.updated_at, l.rev
         FROM lists l LEFT JOIN shops s ON s.id = l.shop_id JOIN users u ON u.id = l.owner_id
         WHERE {VISIBLE_LISTS} AND {cond}
         ORDER BY l.kind = 'inbox' DESC, s.position, l.position, l.id"
    )
}

pub async fn load_list(
    conn: &mut SqliteConnection,
    user_id: &str,
    id: &str,
) -> sqlx::Result<Option<ShoppingList>> {
    sqlx::query_as(&list_select("l.id = ?2"))
        .bind(user_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Lists visible to the user (of one shop, or all), changed after rev `since`.
pub async fn visible_lists(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: Option<&str>,
    since: i64,
) -> sqlx::Result<Vec<ShoppingList>> {
    sqlx::query_as(&list_select(
        "(?2 IS NULL OR l.shop_id = ?2) AND l.rev > ?3",
    ))
    .bind(user_id)
    .bind(shop_id)
    .bind(since)
    .fetch_all(conn)
    .await
}

async fn bump_shop(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query("UPDATE shops SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

async fn bump_list(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query("UPDATE lists SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

/// The user's inbox (created on first use): where items without a list arrive.
pub async fn inbox_id(conn: &mut SqliteConnection, user_id: &str) -> sqlx::Result<String> {
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM lists WHERE owner_id = ? AND kind = 'inbox' AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(id) = found {
        return Ok(id);
    }
    let id = crate::util::new_id();
    let rev = next_rev(conn).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO lists (id, shop_id, owner_id, kind, name, position, created_at, updated_at, rev)
         VALUES (?, NULL, ?, 'inbox', 'Inbox', 'V', ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(conn)
    .await?;
    Ok(id)
}

/// The kind of a live list, if it exists.
pub async fn list_kind(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT kind FROM lists WHERE id = ? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Insert a list or template into a shop. With a template its categories and items are
/// copied (the items not ticked); without one, the categories of the shop's most recent list
/// or template, or the starter set for a shop's first (D-7).
async fn insert_list(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: &str,
    list_id: &str,
    list_name: &str,
    kind: &str,
    template: Option<&str>,
) -> ApiResult<()> {
    let from: Option<String> =
        match template {
            Some(t) => Some(t.to_string()),
            None => sqlx::query_scalar(
                "SELECT id FROM lists WHERE shop_id = ? AND deleted_at IS NULL AND kind != 'inbox'
             ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(shop_id)
            .fetch_optional(&mut *conn)
            .await?,
        };
    let names: Vec<String> = match &from {
        Some(d) => {
            sqlx::query_scalar(
                "SELECT name FROM categories WHERE list_id = ? AND deleted_at IS NULL ORDER BY position, id",
            )
            .bind(d)
            .fetch_all(&mut *conn)
            .await?
        }
        None => shopping::STARTER_CATEGORIES
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    let position = next_position(conn, "lists", "shop_id", shop_id).await?;
    let rev = next_rev(conn).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO lists (id, shop_id, owner_id, kind, name, position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(list_id)
    .bind(shop_id)
    .bind(user_id)
    .bind(kind)
    .bind(list_name)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *conn)
    .await
    .map_err(insert_err)?;
    let mut pos: Option<String> = None;
    for n in names {
        let p = order::key_after(pos.as_deref());
        sqlx::query(
            "INSERT INTO categories (id, list_id, name, position, created_at, updated_at, rev)
             VALUES (?,?,?,?,?,?,?)",
        )
        .bind(crate::util::new_id())
        .bind(list_id)
        .bind(n)
        .bind(&p)
        .bind(&ts)
        .bind(&ts)
        .bind(rev)
        .execute(&mut *conn)
        .await?;
        pos = Some(p);
    }
    if let Some(t) = template {
        // The template's items, in its categories of the same name, in its order.
        sqlx::query(
            "INSERT INTO items (id, list_id, category_id, catalog_item_id, product_id, name, amount, unit, step, note,
                                position, added_by, source, created_at, updated_at, rev)
             SELECT upper(hex(randomblob(13))), ?1,
                    (SELECT n.id FROM categories n JOIN categories o ON lower(o.name) = lower(n.name)
                     WHERE n.list_id = ?1 AND n.deleted_at IS NULL AND o.id = i.category_id LIMIT 1),
                    i.catalog_item_id, i.product_id, i.name, i.amount, i.unit, i.step, i.note,
                    i.position, ?2, 'staple', ?3, ?3, ?4
             FROM items i WHERE i.list_id = ?5 AND i.deleted_at IS NULL",
        )
        .bind(list_id)
        .bind(user_id)
        .bind(&ts)
        .bind(rev)
        .bind(t)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Soft-delete lists matching `cond` (bound to `id`) with everything on them.
async fn tombstone_lists(conn: &mut SqliteConnection, cond: &str, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    let ts = now();
    for table in ["items", "categories"] {
        sqlx::query(&format!(
            "UPDATE {table} SET deleted_at = ?1, updated_at = ?1, rev = ?2
             WHERE deleted_at IS NULL AND list_id IN (SELECT id FROM lists WHERE {cond})"
        ))
        .bind(&ts)
        .bind(rev)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(&format!(
        "UPDATE lists SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE deleted_at IS NULL AND {cond}"
    ))
    .bind(&ts)
    .bind(rev)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ---- shops -------------------------------------------------------------------------------

#[utoipa::path(get, path = "/shops", tag = "shopping", summary = "Your shops", responses((status = 200, body = Vec<Shop>), (status = 401, body = crate::error::Problem)))]
pub async fn list_shops(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Shop>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(user_shops(&mut conn, user.id(), 0).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateShop {
    /// Optional client-chosen ULID.
    id: Option<String>,
    name: String,
    /// `#rrggbb`.
    color: Option<String>,
    /// Optional client-chosen ULID for the shop's first list.
    list_id: Option<String>,
}

/// A shop with its lists and their categories.
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ShopBundle {
    pub shop: Shop,
    pub lists: Vec<ShoppingList>,
    pub categories: Vec<Category>,
}

async fn shop_bundle(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: &str,
) -> ApiResult<ShopBundle> {
    let shop = load_shop(conn, shop_id).await?.ok_or(AppError::NotFound)?;
    let lists = visible_lists(conn, user_id, Some(shop_id), 0).await?;
    let categories = items::visible_categories(conn, user_id, Some(shop_id), 0).await?;
    Ok(ShopBundle {
        shop,
        lists,
        categories,
    })
}

#[utoipa::path(post, path = "/shops", tag = "shopping", summary = "Create a shop with a first shopping list", request_body = CreateShop, responses((status = 200, body = ShopBundle), (status = 400, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn create_shop(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateShop>,
) -> ApiResult<Json<ShopBundle>> {
    let id = id_or_new(c.id)?;
    let list_id = id_or_new(c.list_id)?;
    let sname = name("name", &c.name, shopping::MAX_NAME_CHARS)?;
    let color = shopping::check_color(c.color.as_deref().unwrap_or("")).map_err(bad)?;
    let mut tx = state.db.write.begin().await?;
    // Retrying a create returns what the first attempt made.
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM shops WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(owner) = owner {
        return if owner == user.id() {
            Ok(Json(shop_bundle(&mut tx, user.id(), &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    let position = {
        let last: Option<String> =
            sqlx::query_scalar("SELECT MAX(position) FROM shops WHERE owner_id = ?")
                .bind(user.id())
                .fetch_one(&mut *tx)
                .await?;
        order::key_after(last.as_deref())
    };
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO shops (id, owner_id, name, color, position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(user.id())
    .bind(&sname)
    .bind(&color)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    insert_list(
        &mut tx,
        user.id(),
        &id,
        &list_id,
        shopping::DEFAULT_LIST_NAME,
        "list",
        None,
    )
    .await?;
    let bundle = shop_bundle(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(bundle))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchShop {
    name: Option<String>,
    /// `#rrggbb`, or null to clear.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    color: Option<Option<String>>,
}

#[utoipa::path(patch, path = "/shops/{id}", tag = "shopping", summary = "Rename or recolour a shop", params(("id" = String, Path, description = "ULID")), request_body = PatchShop, responses((status = 200, body = Shop), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_shop(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchShop>,
) -> ApiResult<Json<Shop>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_shop(&mut tx, &user, &id).await?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE shops SET name = ? WHERE id = ?")
            .bind(name("name", &n, shopping::MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(c) = p.color {
        let c = shopping::check_color(c.as_deref().unwrap_or("")).map_err(bad)?;
        sqlx::query("UPDATE shops SET color = ? WHERE id = ?")
            .bind(c)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump_shop(&mut tx, &id).await?;
    let shop = load_shop(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(shop))
}

#[utoipa::path(delete, path = "/shops/{id}", tag = "shopping", summary = "Delete a shop with all its lists and templates", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete_shop(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_shop(&mut tx, &user, &id).await?;
    tombstone_lists(&mut tx, "shop_id = ?3", &id).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "UPDATE shops SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3 AND deleted_at IS NULL",
    )
    .bind(&ts)
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM catalog_shop_uses WHERE shop_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE catalog_items SET shop_id = NULL WHERE shop_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- lists -------------------------------------------------------------------------------

#[utoipa::path(get, path = "/lists", tag = "shopping", summary = "Lists, templates and your inbox, with their open item counts", responses((status = 200, body = Vec<ShoppingList>), (status = 401, body = crate::error::Problem)))]
pub async fn list_lists(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<ShoppingList>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(visible_lists(&mut conn, user.id(), None, 0).await?))
}

#[utoipa::path(get, path = "/lists/{id}", tag = "shopping", summary = "One list", params(("id" = String, Path, description = "ULID")), responses((status = 200, body = ShoppingList), (status = 404, body = crate::error::Problem)))]
pub async fn get_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ShoppingList>> {
    let mut conn = state.db.read.acquire().await?;
    load_list(&mut conn, user.id(), &id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateList {
    /// Optional client-chosen ULID.
    id: Option<String>,
    shop_id: String,
    name: String,
    /// `list` (default) or `template`.
    kind: Option<String>,
    /// Start from this template: its categories and items are copied.
    template_id: Option<String>,
}

/// A list with its categories.
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ListBundle {
    pub list: ShoppingList,
    pub categories: Vec<Category>,
}

async fn list_bundle(
    conn: &mut SqliteConnection,
    user_id: &str,
    id: &str,
) -> ApiResult<ListBundle> {
    let list = load_list(conn, user_id, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let categories = items::list_categories(conn, id).await?;
    Ok(ListBundle { list, categories })
}

#[utoipa::path(post, path = "/lists", tag = "shopping", summary = "Create a list or template in one of your shops, empty (with the categories of the shop's latest list) or from a template", request_body = CreateList, responses((status = 200, body = ListBundle), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn create_list(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateList>,
) -> ApiResult<Json<ListBundle>> {
    let id = id_or_new(c.id)?;
    let lname = name("name", &c.name, shopping::MAX_NAME_CHARS)?;
    let kind = c.kind.as_deref().unwrap_or("list");
    if kind != "list" && kind != "template" {
        return Err(bad("kind must be list or template"));
    }
    let mut tx = state.db.write.begin().await?;
    if let Some(l) = load_list(&mut tx, user.id(), &id).await? {
        return if l.shop_id.as_deref() == Some(c.shop_id.as_str()) {
            Ok(Json(list_bundle(&mut tx, user.id(), &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    visibility::require_shop(&mut tx, &user, &c.shop_id).await?;
    if let Some(t) = &c.template_id {
        // Any template you can see (also one shared with you), never another kind of list.
        visibility::require_list(&mut tx, &user, t, Need::Read).await?;
        if list_kind(&mut tx, t).await?.as_deref() != Some("template") {
            return Err(bad("that isn't a template"));
        }
    }
    insert_list(
        &mut tx,
        user.id(),
        &c.shop_id,
        &id,
        &lname,
        kind,
        c.template_id.as_deref(),
    )
    .await?;
    let bundle = list_bundle(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(bundle))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchList {
    name: Option<String>,
    archived: Option<bool>,
}

#[utoipa::path(patch, path = "/lists/{id}", tag = "shopping", summary = "Rename or archive a list or template (owner)", params(("id" = String, Path, description = "ULID")), request_body = PatchList, responses((status = 200, body = ShoppingList), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchList>,
) -> ApiResult<Json<ShoppingList>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &id, Need::Manage).await?;
    not_inbox(&mut tx, &id).await?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE lists SET name = ? WHERE id = ?")
            .bind(name("name", &n, shopping::MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(a) = p.archived {
        sqlx::query("UPDATE lists SET archived_at = CASE WHEN ? THEN COALESCE(archived_at, ?) END WHERE id = ?")
            .bind(a)
            .bind(now())
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump_list(&mut tx, &id).await?;
    let list = load_list(&mut tx, user.id(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(list))
}

#[utoipa::path(delete, path = "/lists/{id}", tag = "shopping", summary = "Delete a list or template with its items (owner)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 400, description = "The inbox can't be deleted", body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &id, Need::Manage).await?;
    not_inbox(&mut tx, &id).await?;
    tombstone_lists(&mut tx, "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/shops/{id}/move", tag = "shopping", summary = "Reorder a shop (after or before another of your shops; neither = last)", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = Shop), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_shop(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<Shop>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_shop(&mut tx, &user, &id).await?;
    let scope = Scope {
        table: "shops",
        column: "owner_id",
        id: user.id(),
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE shops SET position = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(key)
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let shop = load_shop(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(shop))
}

#[utoipa::path(post, path = "/lists/{id}/move", tag = "shopping", summary = "Reorder a list within its shop (owner)", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = ShoppingList), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<ShoppingList>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &id, Need::Manage).await?;
    not_inbox(&mut tx, &id).await?;
    let shop_id: String = sqlx::query_scalar("SELECT shop_id FROM lists WHERE id = ?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    let scope = Scope {
        table: "lists",
        column: "shop_id",
        id: &shop_id,
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE lists SET position = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(key)
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let list = load_list(&mut tx, user.id(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(list))
}

/// The inbox stays as it is: not renamed, archived, deleted, moved or shared.
pub async fn not_inbox(conn: &mut SqliteConnection, id: &str) -> ApiResult<()> {
    if list_kind(conn, id).await?.as_deref() == Some("inbox") {
        return Err(bad("the inbox can't be changed like a list"));
    }
    Ok(())
}
