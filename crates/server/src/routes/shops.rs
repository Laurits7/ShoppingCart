//! Shops and lists (SPEC §6.1, §6.2), and the one-request load of the Shopping tab.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use shoppingcart_domain::{order, shopping};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::{catalog, items, staples};
use crate::{
    AppState,
    auth::AuthUser,
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::{
        CatalogItem, Category, Image, Item, Product, ProductPreference, Shop, ShoppingList, Staple,
    },
    positions::{Place, Scope},
    util::{double_option, id_or_new, name, now},
    visibility::{self, Need, ROLE, VISIBLE_LISTS},
};

const SHOP_COLS: &str = "id, name, color, position, default_list_id, created_at, updated_at, rev";

pub async fn load_shop(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Option<Shop>> {
    sqlx::query_as(&format!(
        "SELECT {SHOP_COLS} FROM shops WHERE id = ? AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

async fn user_shops(conn: &mut SqliteConnection, user_id: &str) -> sqlx::Result<Vec<Shop>> {
    sqlx::query_as(&format!(
        "SELECT {SHOP_COLS} FROM shops WHERE owner_id = ? AND deleted_at IS NULL ORDER BY position, id"
    ))
    .bind(user_id)
    .fetch_all(conn)
    .await
}

/// Lists visible to the user bound as `?1`, narrowed by `cond`.
fn list_select(cond: &str) -> String {
    format!(
        "SELECT l.id, l.shop_id, l.owner_id, l.name, l.position, l.archived_at,
                COALESCE(s.default_list_id = l.id, 0) AS is_default, {ROLE} AS role,
                (SELECT COUNT(*) FROM items i
                 WHERE i.list_id = l.id AND i.deleted_at IS NULL AND i.checked_at IS NULL) AS open_count,
                l.created_at, l.updated_at, l.rev
         FROM lists l JOIN shops s ON s.id = l.shop_id
         WHERE {VISIBLE_LISTS} AND {cond}
         ORDER BY s.position, l.position, l.id"
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

async fn visible_lists(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: Option<&str>,
) -> sqlx::Result<Vec<ShoppingList>> {
    sqlx::query_as(&list_select("(?2 IS NULL OR l.shop_id = ?2)"))
        .bind(user_id)
        .bind(shop_id)
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

/// Insert a list into a shop with its categories: a copy of the default list's, or the
/// starter set for a shop's first list (D-7).
async fn insert_list(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: &str,
    list_id: &str,
    list_name: &str,
) -> ApiResult<()> {
    let default_list: Option<Option<String>> =
        sqlx::query_scalar("SELECT default_list_id FROM shops WHERE id = ?")
            .bind(shop_id)
            .fetch_optional(&mut *conn)
            .await?;
    let names: Vec<String> = match default_list.flatten() {
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
        "INSERT INTO lists (id, shop_id, owner_id, name, position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(list_id)
    .bind(shop_id)
    .bind(user_id)
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

// ---- the Shopping tab ------------------------------------------------------------------

/// Everything the Shopping tab shows, in one response.
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ShoppingData {
    pub shops: Vec<Shop>,
    pub lists: Vec<ShoppingList>,
    pub categories: Vec<Category>,
    pub items: Vec<Item>,
    pub staples: Vec<Staple>,
    pub catalog: Vec<CatalogItem>,
    pub products: Vec<Product>,
    pub preferences: Vec<ProductPreference>,
    /// Your pictures in use (for their credits).
    pub images: Vec<Image>,
}

#[utoipa::path(get, path = "/shopping", tag = "shopping", summary = "Everything on your Shopping tab: shops, lists, categories, items, staples, your catalogue and products", responses((status = 200, body = ShoppingData), (status = 401, body = crate::error::Problem)))]
pub async fn shopping(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<ShoppingData>> {
    let mut conn = state.db.read.begin().await?;
    let uid = user.id();
    let shops = user_shops(&mut conn, uid).await?;
    let lists = visible_lists(&mut conn, uid, None).await?;
    let categories = items::visible_categories(&mut conn, uid, None).await?;
    let items = items::visible_items(&mut conn, uid, None).await?;
    let staples = staples::user_staples(&mut conn, uid, None).await?;
    let catalog = catalog::all(&mut conn, uid).await?;
    let products = super::products::user_products(&mut conn, uid, None).await?;
    let preferences = super::products::user_preferences(&mut conn, uid, None).await?;
    let images = sqlx::query_as(
        "SELECT id, width, height, source, source_ref, created_at FROM images WHERE owner_id = ?1
           AND (id IN (SELECT image_id FROM products WHERE user_id = ?1 AND deleted_at IS NULL)
                OR id IN (SELECT image_id FROM catalog_items WHERE user_id = ?1 AND deleted_at IS NULL))",
    )
    .bind(uid)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(ShoppingData {
        images,
        products,
        preferences,
        shops,
        lists,
        categories,
        items,
        staples,
        catalog,
    }))
}

// ---- shops -------------------------------------------------------------------------------

#[utoipa::path(get, path = "/shops", tag = "shopping", summary = "Your shops", responses((status = 200, body = Vec<Shop>), (status = 401, body = crate::error::Problem)))]
pub async fn list_shops(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Shop>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(user_shops(&mut conn, user.id()).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateShop {
    /// Optional client-chosen ULID.
    id: Option<String>,
    name: String,
    /// `#rrggbb`.
    color: Option<String>,
    /// Optional client-chosen ULID for the shop's default list.
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
    let lists = visible_lists(conn, user_id, Some(shop_id)).await?;
    let categories = items::visible_categories(conn, user_id, Some(shop_id)).await?;
    Ok(ShopBundle {
        shop,
        lists,
        categories,
    })
}

#[utoipa::path(post, path = "/shops", tag = "shopping", summary = "Create a shop with its default list", request_body = CreateShop, responses((status = 200, body = ShopBundle), (status = 400, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
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
    )
    .await?;
    sqlx::query("UPDATE shops SET default_list_id = ? WHERE id = ?")
        .bind(&list_id)
        .bind(&id)
        .execute(&mut *tx)
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
    /// Make this list (of the shop, not archived) the default list.
    default_list_id: Option<String>,
}

#[utoipa::path(patch, path = "/shops/{id}", tag = "shopping", summary = "Rename or recolour a shop, or choose its default list", params(("id" = String, Path, description = "ULID")), request_body = PatchShop, responses((status = 200, body = Shop), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
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
    if let Some(list_id) = p.default_list_id {
        set_default(&mut tx, &id, &list_id).await?;
    }
    bump_shop(&mut tx, &id).await?;
    let shop = load_shop(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(shop))
}

async fn set_default(conn: &mut SqliteConnection, shop_id: &str, list_id: &str) -> ApiResult<()> {
    let archived: Option<Option<String>> = sqlx::query_scalar(
        "SELECT archived_at FROM lists WHERE id = ? AND shop_id = ? AND deleted_at IS NULL",
    )
    .bind(list_id)
    .bind(shop_id)
    .fetch_optional(&mut *conn)
    .await?;
    match archived {
        None => return Err(bad("that list isn't in this shop")),
        Some(Some(_)) => return Err(bad("an archived list can't be the default")),
        Some(None) => {}
    }
    let old: Option<String> = sqlx::query_scalar("SELECT default_list_id FROM shops WHERE id = ?")
        .bind(shop_id)
        .fetch_one(&mut *conn)
        .await?;
    sqlx::query("UPDATE shops SET default_list_id = ? WHERE id = ?")
        .bind(list_id)
        .bind(shop_id)
        .execute(&mut *conn)
        .await?;
    // Both lists' `is_default` changed.
    if let Some(old) = old.filter(|o| o != list_id) {
        bump_list(conn, &old).await?;
    }
    bump_list(conn, list_id).await?;
    bump_shop(conn, shop_id).await?;
    Ok(())
}

#[utoipa::path(delete, path = "/shops/{id}", tag = "shopping", summary = "Delete a shop with all its lists, items and staples", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
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
    for table in ["staples", "shops"] {
        let col = if table == "shops" { "id" } else { "shop_id" };
        sqlx::query(&format!(
            "UPDATE {table} SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE {col} = ?3 AND deleted_at IS NULL"
        ))
        .bind(&ts)
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    }
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

#[utoipa::path(get, path = "/lists", tag = "shopping", summary = "Lists you can see, with their open item counts", responses((status = 200, body = Vec<ShoppingList>), (status = 401, body = crate::error::Problem)))]
pub async fn list_lists(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<ShoppingList>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(visible_lists(&mut conn, user.id(), None).await?))
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

#[utoipa::path(post, path = "/lists", tag = "shopping", summary = "Create a list in one of your shops (it copies the default list's categories)", request_body = CreateList, responses((status = 200, body = ListBundle), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn create_list(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateList>,
) -> ApiResult<Json<ListBundle>> {
    let id = id_or_new(c.id)?;
    let lname = name("name", &c.name, shopping::MAX_NAME_CHARS)?;
    let mut tx = state.db.write.begin().await?;
    if let Some(l) = load_list(&mut tx, user.id(), &id).await? {
        return if l.shop_id == c.shop_id {
            Ok(Json(list_bundle(&mut tx, user.id(), &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    visibility::require_shop(&mut tx, &user, &c.shop_id).await?;
    insert_list(&mut tx, user.id(), &c.shop_id, &id, &lname).await?;
    let bundle = list_bundle(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(bundle))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchList {
    name: Option<String>,
    archived: Option<bool>,
    /// `true` makes this the shop's default list.
    is_default: Option<bool>,
}

#[utoipa::path(patch, path = "/lists/{id}", tag = "shopping", summary = "Rename, archive or make a list the default (owner)", params(("id" = String, Path, description = "ULID")), request_body = PatchList, responses((status = 200, body = ShoppingList), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchList>,
) -> ApiResult<Json<ShoppingList>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &id, Need::Manage).await?;
    let list = load_list(&mut tx, user.id(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE lists SET name = ? WHERE id = ?")
            .bind(name("name", &n, shopping::MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(a) = p.archived {
        if a && list.is_default && p.is_default != Some(false) {
            return Err(bad(
                "the default list can't be archived; make another list the default first",
            ));
        }
        sqlx::query("UPDATE lists SET archived_at = CASE WHEN ? THEN COALESCE(archived_at, ?) END WHERE id = ?")
            .bind(a)
            .bind(now())
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    match p.is_default {
        Some(true) => set_default(&mut tx, &list.shop_id, &id).await?,
        Some(false) if list.is_default => {
            return Err(bad(
                "make another list the default instead (every shop has one)",
            ));
        }
        _ => {}
    }
    bump_list(&mut tx, &id).await?;
    let list = load_list(&mut tx, user.id(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(list))
}

#[utoipa::path(delete, path = "/lists/{id}", tag = "shopping", summary = "Delete a list with its items (owner; not the default list)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, description = "It's the shop's default list", body = crate::error::Problem)))]
pub async fn delete_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &id, Need::Manage).await?;
    let list = load_list(&mut tx, user.id(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if list.is_default {
        return Err(AppError::Conflict(
            "the default list can't be deleted; make another list the default first".into(),
        ));
    }
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
