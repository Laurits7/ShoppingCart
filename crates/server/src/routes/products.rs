//! Products: branded versions of a catalogue item, with pictures, and the preferred one
//! everywhere or per shop (SPEC §6.4b). Per user, like the catalogue (D-21).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use shoppingcart_domain::{products, shopping::MAX_NAME_CHARS};
use sqlx::SqliteConnection;

use crate::{
    AppState,
    auth::AuthUser,
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::{Product, ProductPreference},
    util::{double_option, id_or_new, now},
    visibility,
};

const COLS: &str = "id, catalog_item_id, brand, name, barcode, image_id, source, source_ref, position, created_at, updated_at, rev";

/// The user's live products, optionally of one catalogue item.
pub async fn user_products(
    conn: &mut SqliteConnection,
    user_id: &str,
    catalog_item_id: Option<&str>,
) -> sqlx::Result<Vec<Product>> {
    sqlx::query_as(&format!(
        "SELECT {COLS} FROM products WHERE user_id = ?1 AND deleted_at IS NULL
           AND (?2 IS NULL OR catalog_item_id = ?2)
         ORDER BY catalog_item_id, position, id"
    ))
    .bind(user_id)
    .bind(catalog_item_id)
    .fetch_all(conn)
    .await
}

/// The user's product preferences, optionally of one catalogue item.
pub async fn user_preferences(
    conn: &mut SqliteConnection,
    user_id: &str,
    catalog_item_id: Option<&str>,
) -> sqlx::Result<Vec<ProductPreference>> {
    sqlx::query_as(
        "SELECT p.catalog_item_id, NULLIF(p.shop_id, '') AS shop_id, p.product_id
         FROM product_preferences p JOIN catalog_items c ON c.id = p.catalog_item_id
         WHERE c.user_id = ?1 AND c.deleted_at IS NULL AND (?2 IS NULL OR p.catalog_item_id = ?2)
         ORDER BY p.catalog_item_id, p.shop_id",
    )
    .bind(user_id)
    .bind(catalog_item_id)
    .fetch_all(conn)
    .await
}

pub async fn load(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Product> {
    sqlx::query_as(&format!("SELECT {COLS} FROM products WHERE id = ?"))
        .bind(id)
        .fetch_one(conn)
        .await
}

fn text(field: &str, s: Option<&str>) -> ApiResult<String> {
    let s = s.unwrap_or("").trim();
    if s.chars().count() > MAX_NAME_CHARS || s.chars().any(char::is_control) {
        return Err(bad(format!(
            "{field} must be at most {MAX_NAME_CHARS} characters"
        )));
    }
    Ok(s.to_string())
}

fn barcode(b: Option<&str>) -> ApiResult<Option<String>> {
    b.filter(|b| !b.trim().is_empty())
        .map(|b| products::check_barcode(b).map_err(bad))
        .transpose()
}

#[utoipa::path(get, path = "/catalog/{id}/products", tag = "products", summary = "Products (brands) of a catalogue item", params(("id" = String, Path, description = "Catalogue item ULID")), responses((status = 200, body = Vec<Product>), (status = 404, body = crate::error::Problem)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Product>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_catalog_item(&mut conn, &user, &id).await?;
    Ok(Json(user_products(&mut conn, user.id(), Some(&id)).await?))
}

/// What a new product is; brand or name (or both) must be given.
#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct NewProduct {
    /// Optional client-chosen ULID.
    pub id: Option<String>,
    pub brand: Option<String>,
    pub name: Option<String>,
    /// EAN/UPC, 8 to 14 digits.
    pub barcode: Option<String>,
    /// A picture you uploaded (`POST /images`).
    pub image_id: Option<String>,
}

/// Create a product of a catalogue item (the caller has checked the item is the user's).
pub async fn insert(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    catalog_item_id: &str,
    p: NewProduct,
    source: &str,
    source_ref: Option<&str>,
) -> ApiResult<Product> {
    let id = id_or_new(p.id)?;
    let brand = text("brand", p.brand.as_deref())?;
    let pname = text("name", p.name.as_deref())?;
    if brand.is_empty() && pname.is_empty() {
        return Err(bad("give a brand or a name"));
    }
    let code = barcode(p.barcode.as_deref())?;
    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT user_id, catalog_item_id FROM products WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some((owner, cat)) = existing {
        return if owner == user.id() && cat == catalog_item_id {
            Ok(load(conn, &id).await?)
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    if let Some(img) = &p.image_id {
        visibility::require_own_image(conn, user, img).await?;
    }
    let position = next_position(conn, "products", "catalog_item_id", catalog_item_id).await?;
    let rev = next_rev(conn).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO products (id, user_id, catalog_item_id, brand, name, barcode, image_id, source, source_ref,
                               position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(user.id())
    .bind(catalog_item_id)
    .bind(&brand)
    .bind(&pname)
    .bind(&code)
    .bind(&p.image_id)
    .bind(source)
    .bind(source_ref)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *conn)
    .await
    .map_err(insert_err)?;
    Ok(load(conn, &id).await?)
}

#[utoipa::path(post, path = "/catalog/{id}/products", tag = "products", summary = "Add a product (brand) to a catalogue item", params(("id" = String, Path, description = "Catalogue item ULID")), request_body = NewProduct, responses((status = 200, body = Product), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<NewProduct>,
) -> ApiResult<Json<Product>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_catalog_item(&mut tx, &user, &id).await?;
    let product = insert(&mut tx, &user, &id, p, "manual", None).await?;
    tx.commit().await?;
    Ok(Json(product))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchProduct {
    brand: Option<String>,
    name: Option<String>,
    /// Null to clear.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    barcode: Option<Option<String>>,
    /// A picture you uploaded, or null for none.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    image_id: Option<Option<String>>,
}

#[utoipa::path(patch, path = "/products/{id}", tag = "products", summary = "Edit a product", params(("id" = String, Path, description = "ULID")), request_body = PatchProduct, responses((status = 200, body = Product), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchProduct>,
) -> ApiResult<Json<Product>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_product(&mut tx, &user, &id).await?;
    let cur = load(&mut tx, &id).await?;
    let brand = match p.brand {
        Some(b) => text("brand", Some(&b))?,
        None => cur.brand,
    };
    let pname = match p.name {
        Some(n) => text("name", Some(&n))?,
        None => cur.name,
    };
    if brand.is_empty() && pname.is_empty() {
        return Err(bad("give a brand or a name"));
    }
    let code = match p.barcode {
        Some(b) => barcode(b.as_deref())?,
        None => cur.barcode,
    };
    let image = match p.image_id {
        Some(Some(img)) => {
            visibility::require_own_image(&mut tx, &user, &img).await?;
            Some(img)
        }
        Some(None) => None,
        None => cur.image_id,
    };
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "UPDATE products SET brand = ?, name = ?, barcode = ?, image_id = ?, updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&brand)
    .bind(&pname)
    .bind(&code)
    .bind(&image)
    .bind(now())
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    let product = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(product))
}

/// Forget products matching `cond` (bound to `id`): items and staples pinned to them become
/// generic, preferences for them go.
pub async fn tombstone(conn: &mut SqliteConnection, cond: &str, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    let ts = now();
    let gone = format!("SELECT id FROM products WHERE {cond}");
    for table in ["items", "staples"] {
        sqlx::query(&format!(
            "UPDATE {table} SET product_id = NULL, updated_at = ?1, rev = ?2 WHERE product_id IN ({gone})"
        ))
        .bind(&ts)
        .bind(rev)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(&format!(
        "DELETE FROM product_preferences WHERE product_id IN ({gone})"
    ))
    .bind(&ts)
    .bind(rev)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    sqlx::query(&format!(
        "UPDATE products SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE deleted_at IS NULL AND {cond}"
    ))
    .bind(&ts)
    .bind(rev)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[utoipa::path(delete, path = "/products/{id}", tag = "products", summary = "Delete a product; items pinned to it become generic", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_product(&mut tx, &user, &id).await?;
    tombstone(&mut tx, "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetPreferred {
    /// The product to prefer, or null for no preference.
    product_id: Option<String>,
    /// Prefer it at this shop only; absent for everywhere.
    shop_id: Option<String>,
}

#[utoipa::path(put, path = "/catalog/{id}/preferred", tag = "products", summary = "Set (or clear) the preferred product, everywhere or at one shop", params(("id" = String, Path, description = "Catalogue item ULID")), request_body = SetPreferred, responses((status = 200, description = "The item's preferences now", body = Vec<ProductPreference>), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn set_preferred(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(s): Json<SetPreferred>,
) -> ApiResult<Json<Vec<ProductPreference>>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_catalog_item(&mut tx, &user, &id).await?;
    if let Some(shop) = &s.shop_id {
        visibility::require_shop(&mut tx, &user, shop).await?;
    }
    let shop = s.shop_id.unwrap_or_default();
    match s.product_id {
        Some(p) => {
            let of = visibility::require_product(&mut tx, &user, &p).await?;
            if of != id {
                return Err(bad("that product belongs to another catalogue item"));
            }
            sqlx::query(
                "INSERT INTO product_preferences (catalog_item_id, shop_id, product_id) VALUES (?,?,?)
                 ON CONFLICT (catalog_item_id, shop_id) DO UPDATE SET product_id = excluded.product_id",
            )
            .bind(&id)
            .bind(&shop)
            .bind(&p)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "DELETE FROM product_preferences WHERE catalog_item_id = ? AND shop_id = ?",
            )
            .bind(&id)
            .bind(&shop)
            .execute(&mut *tx)
            .await?;
        }
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE catalog_items SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let prefs = user_preferences(&mut tx, user.id(), Some(&id)).await?;
    tx.commit().await?;
    Ok(Json(prefs))
}
