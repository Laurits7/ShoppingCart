//! Settings → Integrations (SPEC §6.10): connect Mealie, pick its list and where items go,
//! sync now, see the last error. The token is write-only.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AppState,
    auth::AuthUser,
    db::next_rev,
    error::{ApiResult, AppError, bad},
    integrations::{self, COLS, Integration, MealieSettings, Row},
    mealie::{Mealie, MealieList},
    net,
    util::{new_id, now},
    visibility::{self, Need},
};

/// An integration of the user's own (others': 404).
async fn own(state: &AppState, user: &AuthUser, id: &str) -> ApiResult<Row> {
    let mut conn = state.db.read.acquire().await?;
    let row = integrations::load(&mut conn, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != user.id() {
        return Err(AppError::NotFound);
    }
    Ok(row)
}

/// Settings as given: a valid address, a choice of what happens in Mealie, and targets the
/// user may add to.
async fn check(
    state: &AppState,
    user: &AuthUser,
    mut s: MealieSettings,
) -> ApiResult<MealieSettings> {
    s.base_url = net::base_url(&s.base_url)?;
    if !matches!(s.after_import.as_str(), "check" | "delete" | "keep") {
        return Err(bad("after_import must be check, delete or keep"));
    }
    let mut conn = state.db.read.acquire().await?;
    if let Some(l) = &s.target_list_id {
        visibility::require_list(&mut conn, user, l, Need::EditItems)
            .await
            .map_err(|_| bad("you can't add to that list"))?;
    }
    if let Some(shop) = &s.target_shop_id {
        visibility::require_shop(&mut conn, user, shop)
            .await
            .map_err(|_| bad("no such shop"))?;
    }
    Ok(s)
}

#[utoipa::path(get, path = "/integrations", tag = "integrations", summary = "Your integrations (tokens are never shown)", responses((status = 200, body = Vec<Integration>)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Integration>>> {
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT {COLS} FROM integrations WHERE owner_id = ? AND deleted_at IS NULL ORDER BY created_at"
    ))
    .bind(user.id())
    .fetch_all(&state.db.read)
    .await?;
    Ok(Json(rows.iter().map(Row::view).collect()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewIntegration {
    /// `mealie`.
    provider: String,
    settings: MealieSettings,
    /// The Mealie API token (stored encrypted, never shown again).
    secret: String,
    #[serde(default)]
    enabled: bool,
}

#[utoipa::path(post, path = "/integrations", tag = "integrations", summary = "Connect Mealie (browser session only)", request_body = NewIntegration, responses((status = 200, body = Integration), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(n): Json<NewIntegration>,
) -> ApiResult<Json<Integration>> {
    // Integration tokens are credentials: managed from a signed-in browser only (D-25).
    user.require_session()?;
    if n.provider != "mealie" {
        return Err(bad("provider must be mealie"));
    }
    let secret = n.secret.trim();
    if secret.is_empty() || secret.len() > 4096 {
        return Err(bad("give the Mealie API token"));
    }
    let settings = check(&state, &user, n.settings).await?;
    let id = new_id();
    let ts = now();
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "INSERT INTO integrations (id, owner_id, provider, settings, secret, enabled, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(user.id())
    .bind(&n.provider)
    .bind(serde_json::to_string(&settings)?)
    .bind(state.secrets.seal(secret))
    .bind(n.enabled)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(own(&state, &user, &id).await?.view()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchIntegration {
    settings: Option<MealieSettings>,
    /// A new token.
    secret: Option<String>,
    enabled: Option<bool>,
}

#[utoipa::path(patch, path = "/integrations/{id}", tag = "integrations", summary = "Change settings, the token, or switch it on/off (browser session only)", params(("id" = String, Path, description = "ULID")), request_body = PatchIntegration, responses((status = 200, body = Integration), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchIntegration>,
) -> ApiResult<Json<Integration>> {
    user.require_session()?;
    let row = own(&state, &user, &id).await?;
    let settings = match p.settings {
        Some(s) => check(&state, &user, s).await?,
        None => row.settings(),
    };
    let secret = match p.secret.as_deref().map(str::trim) {
        Some("") | None => row.secret.clone(),
        Some(t) if t.len() > 4096 => return Err(bad("that token is too long")),
        Some(t) => Some(state.secrets.seal(t)),
    };
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    // Changed settings: try again soon, whatever the back-off was.
    sqlx::query(
        "UPDATE integrations SET settings = ?, secret = ?, enabled = ?, failures = 0, next_run_at = NULL,
                                 updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(serde_json::to_string(&settings)?)
    .bind(&secret)
    .bind(p.enabled.unwrap_or(row.enabled))
    .bind(now())
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(own(&state, &user, &id).await?.view()))
}

#[utoipa::path(delete, path = "/integrations/{id}", tag = "integrations", summary = "Disconnect (the token is deleted)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    user.require_session()?;
    own(&state, &user, &id).await?;
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE integrations SET secret = NULL, enabled = 0, deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct SyncOutcome {
    /// Items imported this time.
    #[ts(type = "number")]
    pub imported: i64,
    /// What went wrong, if anything (also kept as `last_error`).
    pub error: Option<String>,
    pub integration: Integration,
}

#[utoipa::path(post, path = "/integrations/{id}/sync", tag = "integrations", summary = "Sync now", params(("id" = String, Path, description = "ULID")), responses((status = 200, body = SyncOutcome), (status = 404, body = crate::error::Problem)))]
pub async fn sync_now(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<SyncOutcome>> {
    own(&state, &user, &id).await?;
    let o = integrations::run(&state, &id).await?;
    let integration = own(&state, &user, &id).await?.view();
    Ok(Json(SyncOutcome {
        imported: o.imported,
        error: o.error,
        integration,
    }))
}

#[utoipa::path(get, path = "/integrations/{id}/mealie/lists", tag = "integrations", summary = "The Mealie shopping lists the stored token can see (to pick one)", params(("id" = String, Path, description = "ULID")), responses((status = 200, body = Vec<MealieList>), (status = 404, body = crate::error::Problem), (status = 503, description = "Mealie can't be reached or refused the token", body = crate::error::Problem)))]
pub async fn mealie_lists(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<MealieList>>> {
    let row = own(&state, &user, &id).await?;
    let token = row
        .secret
        .as_deref()
        .and_then(|t| state.secrets.open(t))
        .ok_or_else(|| bad("enter the Mealie token again"))?;
    let s = row.settings();
    let m = Mealie {
        http: &state.integration_http,
        base: &s.base_url,
        token: &token,
    };
    m.lists().await.map(Json).map_err(AppError::Unavailable)
}
