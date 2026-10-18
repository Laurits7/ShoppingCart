//! Backups (admins) and the per-user export (SPEC §6.14).

use axum::{
    Json,
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use serde::Serialize;

use crate::{
    AppState,
    auth::AuthUser,
    backup::{self, Backup},
    error::{ApiResult, AppError},
    models::Me,
};

/// Backups hold everyone's data: admins only, from a signed-in browser.
fn require(user: &AuthUser) -> ApiResult<()> {
    user.require_admin()?;
    user.require_session()
}

#[utoipa::path(get, path = "/admin/backups", tag = "admin", summary = "Backups in data/backups (admins, browser session)", responses((status = 200, body = Vec<Backup>), (status = 403, body = crate::error::Problem)))]
pub async fn backups(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Backup>>> {
    require(&user)?;
    Ok(Json(backup::list(&state.config.data_dir)))
}

#[utoipa::path(post, path = "/admin/backups", tag = "admin", summary = "Make a backup now", responses((status = 200, body = Vec<Backup>), (status = 403, body = crate::error::Problem)))]
pub async fn backup_now(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Backup>>> {
    require(&user)?;
    backup::make(&state).await?;
    Ok(Json(backup::list(&state.config.data_dir)))
}

#[utoipa::path(get, path = "/admin/backups/{name}", tag = "admin", summary = "Download a backup (an SQLite file)", params(("name" = String, Path, description = "As listed")), responses((status = 200, description = "The database file"), (status = 404, body = crate::error::Problem)))]
pub async fn download(
    State(state): State<AppState>,
    user: AuthUser,
    Path(name): Path<String>,
) -> ApiResult<impl IntoResponse> {
    require(&user)?;
    if !backup::valid_name(&name) {
        return Err(AppError::NotFound);
    }
    let bytes = tokio::fs::read(backup::dir(&state.config.data_dir).join(&name))
        .await
        .map_err(|_| AppError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.sqlite3".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        bytes,
    ))
}

/// Everything that is yours, as JSON.
#[derive(Serialize, utoipa::ToSchema)]
pub struct Export {
    format: &'static str,
    exported_at: String,
    #[schema(value_type = Object)]
    me: Me,
    #[schema(value_type = Object)]
    data: serde_json::Value,
}

async fn rows(state: &AppState, sql: &str, uid: &str) -> ApiResult<serde_json::Value> {
    // Each row as a JSON object, built in SQLite so every column comes along.
    let rows: Vec<String> = sqlx::query_scalar(sql)
        .bind(uid)
        .fetch_all(&state.db.read)
        .await?;
    Ok(serde_json::Value::Array(
        rows.iter()
            .filter_map(|r| serde_json::from_str(r).ok())
            .collect(),
    ))
}

fn json_obj(table: &str, alias: &str, cols: &[&str]) -> String {
    let fields = cols
        .iter()
        .map(|c| format!("'{c}', {alias}.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("SELECT json_object({fields}) FROM {table} {alias}")
}

#[utoipa::path(get, path = "/export", tag = "account", summary = "Download what's yours (shops, lists, templates, catalogue, products, wishlists) as JSON", responses((status = 200, body = Export)))]
pub async fn export(State(state): State<AppState>, user: AuthUser) -> ApiResult<impl IntoResponse> {
    let uid = user.id();
    let live = "deleted_at IS NULL";
    let mut data = serde_json::Map::new();
    let tables: Vec<(&str, String)> = vec![
        (
            "shops",
            format!(
                "{} WHERE s.owner_id = ?1 AND s.{live}",
                json_obj(
                    "shops",
                    "s",
                    &[
                        "id",
                        "name",
                        "color",
                        "position",
                        "default_list_id",
                        "created_at"
                    ]
                )
            ),
        ),
        (
            "lists",
            format!(
                "{} WHERE l.owner_id = ?1 AND l.{live}",
                json_obj(
                    "lists",
                    "l",
                    &[
                        "id",
                        "shop_id",
                        "kind",
                        "name",
                        "position",
                        "archived_at",
                        "created_at"
                    ]
                )
            ),
        ),
        (
            "categories",
            format!(
                "{} JOIN lists l ON l.id = c.list_id WHERE l.owner_id = ?1 AND l.{live} AND c.{live}",
                json_obj("categories", "c", &["id", "list_id", "name", "position"])
            ),
        ),
        (
            "items",
            format!(
                "{} JOIN lists l ON l.id = i.list_id WHERE l.owner_id = ?1 AND l.{live} AND i.{live}",
                json_obj(
                    "items",
                    "i",
                    &[
                        "id",
                        "list_id",
                        "category_id",
                        "catalog_item_id",
                        "product_id",
                        "name",
                        "amount",
                        "unit",
                        "step",
                        "note",
                        "position",
                        "checked_at",
                        "source",
                        "external_ref",
                        "created_at"
                    ]
                )
            ),
        ),
        (
            "catalog",
            format!(
                "{} WHERE c.user_id = ?1 AND c.{live}",
                json_obj(
                    "catalog_items",
                    "c",
                    &[
                        "id",
                        "name",
                        "category_name",
                        "amount",
                        "unit",
                        "step",
                        "use_count",
                        "last_used_at",
                        "hidden",
                        "image_id"
                    ]
                )
            ),
        ),
        (
            "products",
            format!(
                "{} WHERE p.user_id = ?1 AND p.{live}",
                json_obj(
                    "products",
                    "p",
                    &[
                        "id",
                        "catalog_item_id",
                        "brand",
                        "name",
                        "barcode",
                        "image_id",
                        "source",
                        "source_ref"
                    ]
                )
            ),
        ),
        (
            "persons",
            format!(
                "{} WHERE p.owner_id = ?1 AND p.{live}",
                json_obj("persons", "p", &["id", "name", "user_id"])
            ),
        ),
        // Only wishlists you own: a gift list about you is never yours, so never in here.
        (
            "wishlists",
            format!(
                "{} WHERE w.owner_id = ?1 AND w.{live}",
                json_obj(
                    "wishlists",
                    "w",
                    &[
                        "id",
                        "kind",
                        "recipient_person_id",
                        "title",
                        "position",
                        "created_at"
                    ]
                )
            ),
        ),
        (
            "wish_sections",
            format!(
                "{} JOIN wishlists w ON w.id = x.wishlist_id WHERE w.owner_id = ?1 AND w.{live} AND x.{live}",
                json_obj(
                    "wish_sections",
                    "x",
                    &["id", "wishlist_id", "name", "position"]
                )
            ),
        ),
        (
            "wish_items",
            format!(
                "{} JOIN wishlists w ON w.id = x.wishlist_id WHERE w.owner_id = ?1 AND w.{live} AND x.{live}",
                json_obj(
                    "wish_items",
                    "x",
                    &[
                        "id",
                        "wishlist_id",
                        "section_id",
                        "title",
                        "url",
                        "price",
                        "currency",
                        "note",
                        "priority",
                        "position",
                        "received_at",
                        "created_at"
                    ]
                )
            ),
        ),
    ];
    for (name, sql) in tables {
        data.insert(name.to_string(), rows(&state, &sql, uid).await?);
    }
    let body = Export {
        format: "shoppingcart-export-1",
        exported_at: crate::util::now(),
        me: Me::from(&user.user),
        data: serde_json::Value::Object(data),
    };
    let file = format!(
        "shoppingcart-{}-{}.json",
        user.user.username,
        chrono::Utc::now().format("%Y%m%d")
    );
    Ok((
        [
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{file}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        Json(body),
    ))
}
