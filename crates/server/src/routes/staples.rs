//! Staples: each shop's template of usual purchases (SPEC §6.3). Adding them to a list is
//! `POST /lists/{id}/staples` in `items`.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use shoppingcart_domain::{amount, catalog::normalize_name, shopping::MAX_NAME_CHARS};
use sqlx::SqliteConnection;

use super::catalog;
use crate::{
    AppState,
    auth::AuthUser,
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::Staple,
    positions::{Place, Scope},
    util::{double_option, id_or_new, name, now},
    visibility,
};

const COLS: &str = "id, shop_id, catalog_item_id, product_id, name, amount, unit, category_name, position, created_at, updated_at, rev";

/// Live staples of the user's shops, optionally of one shop only.
pub async fn user_staples(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: Option<&str>,
) -> sqlx::Result<Vec<Staple>> {
    sqlx::query_as(&format!(
        "SELECT {} FROM staples s JOIN shops p ON p.id = s.shop_id
         WHERE p.owner_id = ?1 AND p.deleted_at IS NULL AND s.deleted_at IS NULL
           AND (?2 IS NULL OR s.shop_id = ?2)
         ORDER BY s.shop_id, s.position, s.id",
        COLS.split(", ")
            .map(|c| format!("s.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
    .bind(user_id)
    .bind(shop_id)
    .fetch_all(conn)
    .await
}

async fn load(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Staple> {
    sqlx::query_as(&format!("SELECT {COLS} FROM staples WHERE id = ?"))
        .bind(id)
        .fetch_one(conn)
        .await
}

fn category(c: Option<String>) -> ApiResult<Option<String>> {
    c.filter(|c| !c.trim().is_empty())
        .map(|c| name("category", &c, MAX_NAME_CHARS))
        .transpose()
}

#[utoipa::path(get, path = "/shops/{id}/staples", tag = "shopping", summary = "A shop's staples, in order", params(("id" = String, Path, description = "Shop ULID")), responses((status = 200, body = Vec<Staple>), (status = 404, body = crate::error::Problem)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<String>,
) -> ApiResult<Json<Vec<Staple>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_shop(&mut conn, &user, &shop_id).await?;
    Ok(Json(
        user_staples(&mut conn, user.id(), Some(&shop_id)).await?,
    ))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateStaple {
    /// Optional client-chosen ULID.
    id: Option<String>,
    name: String,
    /// Default 1.
    amount: Option<f64>,
    unit: Option<String>,
    /// The category it goes into on a list. Default: the catalogue's.
    category_name: Option<String>,
    /// One of your products, for a staple of a particular brand.
    product_id: Option<String>,
}

#[utoipa::path(post, path = "/shops/{id}/staples", tag = "shopping", summary = "Add a staple to a shop", params(("id" = String, Path, description = "Shop ULID")), request_body = CreateStaple, responses((status = 200, body = Staple), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<String>,
    Json(c): Json<CreateStaple>,
) -> ApiResult<Json<Staple>> {
    let id = id_or_new(c.id)?;
    let sname = name("name", &c.name, MAX_NAME_CHARS)?;
    let amt = amount::check_amount(c.amount.unwrap_or(1.0)).map_err(bad)?;
    let unit = amount::check_unit(c.unit.as_deref().unwrap_or("")).map_err(bad)?;
    let mut tx = state.db.write.begin().await?;
    visibility::require_shop(&mut tx, &user, &shop_id).await?;
    let existing: Option<String> = sqlx::query_scalar("SELECT shop_id FROM staples WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(s) = existing {
        return if s == shop_id {
            Ok(Json(load(&mut tx, &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    if let Some(p) = &c.product_id {
        visibility::require_product(&mut tx, &user, p).await?;
    }
    let known = catalog::lookup(&mut tx, user.id(), &normalize_name(&sname)).await?;
    let cat =
        category(c.category_name)?.or_else(|| known.as_ref().and_then(|k| k.category_name.clone()));
    let position = next_position(&mut tx, "staples", "shop_id", &shop_id).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO staples (id, shop_id, catalog_item_id, product_id, name, amount, unit, category_name, position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&shop_id)
    .bind(known.map(|k| k.id))
    .bind(&c.product_id)
    .bind(&sname)
    .bind(amt)
    .bind(&unit)
    .bind(&cat)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    let staple = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(staple))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchStaple {
    name: Option<String>,
    amount: Option<f64>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    unit: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    category_name: Option<Option<String>>,
    /// One of your products, or null for any brand.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    product_id: Option<Option<String>>,
}

#[utoipa::path(patch, path = "/staples/{id}", tag = "shopping", summary = "Edit a staple", params(("id" = String, Path, description = "ULID")), request_body = PatchStaple, responses((status = 200, body = Staple), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchStaple>,
) -> ApiResult<Json<Staple>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_staple(&mut tx, &user, &id).await?;
    if let Some(n) = p.name {
        let n = name("name", &n, MAX_NAME_CHARS)?;
        let known = catalog::lookup(&mut tx, user.id(), &normalize_name(&n)).await?;
        sqlx::query("UPDATE staples SET name = ?, catalog_item_id = ? WHERE id = ?")
            .bind(&n)
            .bind(known.map(|k| k.id))
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(a) = p.amount {
        sqlx::query("UPDATE staples SET amount = ? WHERE id = ?")
            .bind(amount::check_amount(a).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(u) = p.unit {
        sqlx::query("UPDATE staples SET unit = ? WHERE id = ?")
            .bind(amount::check_unit(u.as_deref().unwrap_or("")).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(pid) = p.product_id {
        if let Some(pid) = &pid {
            visibility::require_product(&mut tx, &user, pid).await?;
        }
        sqlx::query("UPDATE staples SET product_id = ? WHERE id = ?")
            .bind(pid)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(c) = p.category_name {
        sqlx::query("UPDATE staples SET category_name = ? WHERE id = ?")
            .bind(category(c)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE staples SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let staple = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(staple))
}

#[utoipa::path(delete, path = "/staples/{id}", tag = "shopping", summary = "Remove a staple", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_staple(&mut tx, &user, &id).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE staples SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/staples/{id}/move", tag = "shopping", summary = "Reorder a staple within its shop", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = Staple), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_staple(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<Staple>> {
    let mut tx = state.db.write.begin().await?;
    let shop_id = visibility::require_staple(&mut tx, &user, &id).await?;
    let scope = Scope {
        table: "staples",
        column: "shop_id",
        id: &shop_id,
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE staples SET position = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(key)
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let staple = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(staple))
}
