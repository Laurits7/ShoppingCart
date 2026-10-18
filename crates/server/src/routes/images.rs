//! Picture upload and download (SPEC §6.4b, §7.4).

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{AppState, auth::AuthUser, error::ApiResult, images, models::Image, visibility};

#[utoipa::path(post, path = "/images", tag = "products", summary = "Upload a picture (JPEG, PNG or WebP as the request body); it is resized and re-encoded", request_body(content = Vec<u8>, content_type = "image/jpeg"), responses((status = 200, body = Image), (status = 400, description = "Not a picture we accept, or unreadable", body = crate::error::Problem), (status = 413, description = "Larger than MAX_UPLOAD_MB")))]
pub async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    body: Bytes,
) -> ApiResult<Json<Image>> {
    Ok(Json(
        images::store(&state, user.id(), body.to_vec(), "upload", None).await?,
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct Size {
    /// `large` (≈640 px) instead of the thumbnail (≈160 px).
    size: Option<String>,
}

#[utoipa::path(get, path = "/images/{id}", tag = "products", summary = "A picture (JPEG)", params(("id" = String, Path, description = "ULID"), Size), responses((status = 200, description = "image/jpeg"), (status = 404, body = crate::error::Problem)))]
pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<Size>,
) -> ApiResult<impl IntoResponse> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_image(&mut conn, &user, &id).await?;
    drop(conn);
    let bytes = images::read(&state, &id, q.size.as_deref() == Some("large")).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            // A picture id never changes its content.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    ))
}
