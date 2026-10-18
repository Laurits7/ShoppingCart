//! Finding products and pictures on Open Food Facts (SPEC §6.4b) and importing them.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;
use shoppingcart_domain::images::{Kind, sniff};

use super::products::{self, NewProduct};
use crate::{
    AppState,
    auth::AuthUser,
    db::next_rev,
    error::{ApiResult, AppError, bad},
    images,
    lookup::{self, LookupResult},
    models::Product,
    util::now,
    visibility,
};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct Search {
    /// A product name, e.g. "alma piim".
    q: String,
}

#[utoipa::path(get, path = "/lookup", tag = "products", summary = "Search Open Food Facts for products by name", params(Search), responses((status = 200, body = Vec<LookupResult>), (status = 503, description = "Lookups are off, or Open Food Facts is unreachable or busy", body = crate::error::Problem)))]
pub async fn search(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(s): Query<Search>,
) -> ApiResult<Json<Vec<LookupResult>>> {
    Ok(Json(lookup::search(&state, &s.q).await?))
}

#[utoipa::path(get, path = "/lookup/barcode/{code}", tag = "products", summary = "Look up a product by its barcode", params(("code" = String, Path, description = "EAN/UPC")), responses((status = 200, body = LookupResult), (status = 404, description = "Not on Open Food Facts", body = crate::error::Problem), (status = 503, body = crate::error::Problem)))]
pub async fn barcode(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(code): Path<String>,
) -> ApiResult<Json<LookupResult>> {
    lookup::by_code(&state, &code)
        .await?
        .map(|r| Json(lookup::public(r)))
        .ok_or(AppError::NotFound)
}

#[utoipa::path(get, path = "/lookup/picture/{code}", tag = "products", summary = "The small picture of a product just found (fetched by the server)", params(("code" = String, Path, description = "A barcode from a lookup result")), responses((status = 200, description = "The picture"), (status = 404, body = crate::error::Problem)))]
pub async fn picture(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(code): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Only for products found in a lookup in the last day (they're cached).
    let r = lookup::by_code(&state, &code)
        .await?
        .ok_or(AppError::NotFound)?;
    let bytes = lookup::picture(&state, &r, false).await?;
    // Served from our origin only if it really is a picture, with a type we set.
    let mime = match sniff(&bytes) {
        Some(Kind::Jpeg) => "image/jpeg",
        Some(Kind::Png) => "image/png",
        Some(Kind::WebP) => "image/webp",
        None => return Err(AppError::NotFound),
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    ))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct Import {
    /// Your catalogue item it belongs to.
    catalog_item_id: String,
    /// Give the picture (and the barcode, if it has none) to this product of the item
    /// instead of making a new product.
    product_id: Option<String>,
    /// Only use the picture, for the catalogue item itself (no brand).
    #[serde(default)]
    picture_only: bool,
}

/// A product's display name from a lookup: "Piim 2.5%" plus its quantity ("1 l").
fn product_name(r: &LookupResult) -> String {
    let name = if r.quantity.is_empty() || r.name.contains(&r.quantity) {
        r.name.clone()
    } else {
        format!("{} {}", r.name, r.quantity).trim().to_string()
    };
    name.chars().take(200).collect()
}

#[utoipa::path(post, path = "/lookup/{code}/import", tag = "products", summary = "Use a product found on Open Food Facts: as a new product of a catalogue item (with its picture), or just its picture", params(("code" = String, Path, description = "Barcode of a lookup result")), request_body = Import, responses((status = 200, description = "The product (null when only the catalogue item's picture was set)", body = Option<Product>), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 503, body = crate::error::Problem)))]
pub async fn import(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
    Json(i): Json<Import>,
) -> ApiResult<Json<Option<Product>>> {
    {
        let mut conn = state.db.read.acquire().await?;
        visibility::require_catalog_item(&mut conn, &user, &i.catalog_item_id).await?;
        if let Some(p) = &i.product_id {
            let of = visibility::require_product(&mut conn, &user, p).await?;
            if of != i.catalog_item_id {
                return Err(bad("that product belongs to another catalogue item"));
            }
        }
    }
    let r = lookup::by_code(&state, &code)
        .await?
        .ok_or(AppError::NotFound)?;
    // The picture is downloaded and stored like an upload, crediting the product page.
    let image = if r.has_picture {
        match lookup::picture(&state, &r, true).await {
            Ok(bytes) => Some(
                images::store(&state, user.id(), bytes, "openfoodfacts", Some(&r.page_url)).await?,
            ),
            Err(e) => {
                tracing::warn!("could not fetch the picture of {code}: {e:?}");
                None
            }
        }
    } else {
        None
    };
    let image_id = image.map(|i| i.id);
    if (i.picture_only || i.product_id.is_some()) && image_id.is_none() {
        return Err(AppError::Unavailable(
            "Open Food Facts has no picture of this product".into(),
        ));
    }

    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let out = match (&i.product_id, i.picture_only) {
        (None, true) => {
            sqlx::query(
                "UPDATE catalog_items SET image_id = ?, updated_at = ?, rev = ? WHERE id = ?",
            )
            .bind(&image_id)
            .bind(now())
            .bind(rev)
            .bind(&i.catalog_item_id)
            .execute(&mut *tx)
            .await?;
            None
        }
        (Some(p), _) => {
            sqlx::query(
                "UPDATE products SET image_id = ?, barcode = COALESCE(barcode, ?), updated_at = ?, rev = ? WHERE id = ?",
            )
            .bind(&image_id)
            .bind(&r.code)
            .bind(now())
            .bind(rev)
            .bind(p)
            .execute(&mut *tx)
            .await?;
            Some(products::load(&mut tx, p).await?)
        }
        (None, false) => {
            // The same barcode imported again for this item: refresh its picture.
            let same: Option<String> = sqlx::query_scalar(
                "SELECT id FROM products WHERE user_id = ? AND catalog_item_id = ? AND barcode = ? AND deleted_at IS NULL",
            )
            .bind(user.id())
            .bind(&i.catalog_item_id)
            .bind(&r.code)
            .fetch_optional(&mut *tx)
            .await?;
            match same {
                Some(p) => {
                    sqlx::query(
                        "UPDATE products SET image_id = COALESCE(?, image_id), updated_at = ?, rev = ? WHERE id = ?",
                    )
                    .bind(&image_id)
                    .bind(now())
                    .bind(rev)
                    .bind(&p)
                    .execute(&mut *tx)
                    .await?;
                    Some(products::load(&mut tx, &p).await?)
                }
                None => Some(
                    products::insert(
                        &mut tx,
                        &user,
                        &i.catalog_item_id,
                        NewProduct {
                            brand: Some(r.brand.clone()),
                            name: Some(product_name(&r)),
                            barcode: Some(r.code.clone()),
                            image_id: image_id.clone(),
                            ..Default::default()
                        },
                        "openfoodfacts",
                        Some(&r.code),
                    )
                    .await?,
                ),
            }
        }
    };
    tx.commit().await?;
    Ok(Json(out))
}
