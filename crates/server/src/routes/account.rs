//! First-run setup, sign-in, profile, sessions, API tokens and user management.

use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use shoppingcart_domain::account;

use crate::{
    AppState,
    auth::{self, AuthUser},
    db::next_rev,
    error::{ApiResult, AppError, bad},
    models::{ApiToken, Me, SessionInfo, User, UserSummary},
    util::{name, now},
};

#[derive(Serialize, utoipa::ToSchema)]
pub struct SetupStatus {
    pub needs_setup: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct Credentials {
    username: String,
    password: String,
    /// Only used by first-run setup.
    display_name: Option<String>,
}

#[utoipa::path(get, path = "/setup", tag = "account", summary = "Whether first-run setup is needed", security(()), responses((status = 200, body = SetupStatus)))]
pub async fn setup_status(State(state): State<AppState>) -> ApiResult<Json<SetupStatus>> {
    Ok(Json(SetupStatus {
        needs_setup: auth::user_count(&state).await? == 0,
    }))
}

/// Create the first (admin) account. Only possible while there are no users.
#[utoipa::path(post, path = "/setup", tag = "account", summary = "Create the first admin account and sign in", security(()), request_body = Credentials, responses((status = 200, body = Me), (status = 400, body = crate::error::Problem), (status = 403, description = "Setup already done", body = crate::error::Problem)))]
pub async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(c): Json<Credentials>,
) -> ApiResult<Response> {
    auth::check_origin(&state, &headers)?;
    // One setup at a time, so two simultaneous first visits can't both become admin.
    static SETUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = SETUP.lock().await;
    if auth::user_count(&state).await? > 0 {
        return Err(AppError::Forbidden("setup already completed".into()));
    }
    let user = auth::create_user(
        &state,
        &c.username,
        &c.password,
        c.display_name.as_deref(),
        true,
    )
    .await?;
    login_response(&state, &headers, &user).await
}

async fn login_response(state: &AppState, headers: &HeaderMap, user: &User) -> ApiResult<Response> {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let token = auth::create_session(state, &user.id, ua).await?;
    let cookie = auth::session_cookie(state, headers, &token);
    Ok(([(header::SET_COOKIE, cookie)], Json(Me::from(user))).into_response())
}

#[utoipa::path(post, path = "/auth/login", tag = "account", summary = "Sign in (sets the session cookie)", security(()), request_body = Credentials, responses((status = 200, body = Me), (status = 401, description = "Wrong username or password", body = crate::error::Problem), (status = 429, description = "Too many failed attempts", body = crate::error::Problem)))]
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(c): Json<Credentials>,
) -> ApiResult<Response> {
    auth::check_origin(&state, &headers)?;
    let username = account::normalize_username(&c.username);
    let keys = vec![
        format!("u:{username}"),
        format!("ip:{}", auth::client_ip(&state, &headers, addr.ip())),
    ];
    auth::check_throttle(&state, &keys).await?;
    let user: Option<User> =
        sqlx::query_as("SELECT * FROM users WHERE username = ? AND deleted_at IS NULL")
            .bind(&username)
            .fetch_optional(&state.db.read)
            .await?;
    let ok =
        auth::verify_password_async(c.password, user.as_ref().map(|u| u.password_hash.clone()))
            .await;
    match user {
        Some(user) if ok => {
            // Only the username's count: signing in to your own account mustn't reset the
            // address's count (that would allow guessing other accounts).
            auth::clear_failures(&state, &keys[..1]).await;
            login_response(&state, &headers, &user).await
        }
        _ => Err(AppError::Unauthorized),
    }
}

#[utoipa::path(post, path = "/auth/logout", tag = "account", summary = "Sign out this session", responses((status = 204, description = "Done"), (status = 401, body = crate::error::Problem)))]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
) -> ApiResult<Response> {
    if let Some(hash) = user.session_hash() {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(hash)
            .execute(&state.db.write)
            .await?;
    }
    Ok((
        [(header::SET_COOKIE, auth::clear_cookie(&state, &headers))],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

#[utoipa::path(get, path = "/me", tag = "account", summary = "The signed-in user", responses((status = 200, body = Me), (status = 401, body = crate::error::Problem)))]
pub async fn me(user: AuthUser) -> Json<Me> {
    Json(Me::from(&user.user))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchMe {
    display_name: Option<String>,
    /// BCP 47 language tag, or empty for the browser's language.
    locale: Option<String>,
}

fn check_locale(l: &str) -> ApiResult<()> {
    let ok = l.is_empty()
        || (l.len() <= 35
            && l.split('-').all(|part| {
                !part.is_empty()
                    && part.len() <= 8
                    && part.chars().all(|c| c.is_ascii_alphanumeric())
            }));
    if ok {
        Ok(())
    } else {
        Err(bad(
            "locale must be a language tag like en-GB, or empty for the browser default",
        ))
    }
}

#[utoipa::path(patch, path = "/me", tag = "account", summary = "Update your profile", request_body = PatchMe, responses((status = 200, body = Me), (status = 400, body = crate::error::Problem), (status = 401, body = crate::error::Problem)))]
pub async fn patch_me(
    State(state): State<AppState>,
    user: AuthUser,
    Json(p): Json<PatchMe>,
) -> ApiResult<Json<Me>> {
    let mut u = user.user;
    if let Some(d) = p.display_name {
        u.display_name = name("display name", &d, 100)?;
    }
    if let Some(l) = p.locale {
        check_locale(&l)?;
        u.locale = l;
    }
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "UPDATE users SET display_name = ?, locale = ?, updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&u.display_name)
    .bind(&u.locale)
    .bind(now())
    .bind(rev)
    .bind(&u.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(Me::from(&u)))
}

/// Merge UI preferences: top-level keys in the body replace stored ones; `null` removes one.
#[utoipa::path(patch, path = "/me/prefs", tag = "account", summary = "Merge UI preferences (null removes a key)", request_body = Object, responses((status = 200, body = Me), (status = 400, body = crate::error::Problem), (status = 401, body = crate::error::Problem)))]
pub async fn patch_prefs(
    State(state): State<AppState>,
    user: AuthUser,
    Json(patch): Json<serde_json::Map<String, serde_json::Value>>,
) -> ApiResult<Json<Me>> {
    let mut tx = state.db.write.begin().await?;
    // Re-read inside the transaction so concurrent merges don't lose keys.
    let stored: String = sqlx::query_scalar("SELECT prefs FROM users WHERE id = ?")
        .bind(user.id())
        .fetch_one(&mut *tx)
        .await?;
    let mut prefs: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&stored).unwrap_or_default();
    for (k, v) in patch {
        if k.len() > 100 {
            return Err(bad("preference keys are at most 100 characters"));
        }
        if v.is_null() {
            prefs.remove(&k);
        } else {
            prefs.insert(k, v);
        }
    }
    let json = serde_json::Value::Object(prefs).to_string();
    if json.len() > 32 * 1024 {
        return Err(bad("preferences are limited to 32 KB"));
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE users SET prefs = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(&json)
        .bind(now())
        .bind(rev)
        .bind(user.id())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut u = user.user;
    u.prefs = json;
    Ok(Json(Me::from(&u)))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ChangePassword {
    current_password: String,
    new_password: String,
}

#[utoipa::path(post, path = "/me/password", tag = "account", summary = "Change password (signs out other sessions and revokes API tokens)", request_body = ChangePassword, responses((status = 204, description = "Done"), (status = 400, body = crate::error::Problem), (status = 403, description = "Wrong current password, or not a browser session", body = crate::error::Problem)))]
pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    Json(p): Json<ChangePassword>,
) -> ApiResult<StatusCode> {
    user.require_session()?;
    if !auth::verify_password_async(p.current_password, Some(user.user.password_hash.clone())).await
    {
        return Err(AppError::Forbidden("current password is wrong".into()));
    }
    auth::validate_new_password(&p.new_password)?;
    let hash = auth::hash_password_async(p.new_password).await?;
    let mut tx = state.db.write.begin().await?;
    sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
        .bind(hash)
        .bind(now())
        .bind(user.id())
        .execute(&mut *tx)
        .await?;
    // A new password ends every other way in: other sessions and all API tokens.
    sqlx::query("UPDATE api_tokens SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL")
        .bind(now())
        .bind(user.id())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash <> ?")
        .bind(user.id())
        .bind(user.session_hash().unwrap_or(""))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- sessions -------------------------------------------------------------------

#[utoipa::path(get, path = "/sessions", tag = "account", summary = "Where you're signed in", responses((status = 200, body = Vec<SessionInfo>), (status = 403, description = "Not a browser session", body = crate::error::Problem)))]
pub async fn list_sessions(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<SessionInfo>>> {
    user.require_session()?;
    let mut sessions: Vec<SessionInfo> = sqlx::query_as(
        "SELECT id, created_at, last_seen_at, expires_at, user_agent FROM sessions
         WHERE user_id = ? AND expires_at > ? ORDER BY last_seen_at DESC",
    )
    .bind(user.id())
    .bind(now())
    .fetch_all(&state.db.read)
    .await?;
    let current: Option<String> =
        sqlx::query_scalar("SELECT id FROM sessions WHERE token_hash = ?")
            .bind(user.session_hash().unwrap_or(""))
            .fetch_optional(&state.db.read)
            .await?;
    for s in &mut sessions {
        s.current = Some(&s.id) == current.as_ref();
    }
    Ok(Json(sessions))
}

#[utoipa::path(delete, path = "/sessions/{id}", tag = "account", summary = "Sign out a session", params(("id" = String, Path, description = "Session id")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn revoke_session(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    user.require_session()?;
    let r = sqlx::query("DELETE FROM sessions WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(user.id())
        .execute(&state.db.write)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---- API tokens -----------------------------------------------------------------

/// A new API token. `token` is only ever shown here.
#[derive(Serialize, utoipa::ToSchema)]
pub struct CreatedToken {
    pub token: String,
    pub info: ApiToken,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateToken {
    name: String,
    /// `read` (default) or `write`.
    scope: Option<String>,
}

#[utoipa::path(get, path = "/tokens", tag = "account", summary = "List your API tokens", responses((status = 200, body = Vec<ApiToken>), (status = 403, description = "Not a browser session", body = crate::error::Problem)))]
pub async fn list_tokens(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<ApiToken>>> {
    user.require_session()?;
    let rows = sqlx::query_as(
        "SELECT id, name, scope, created_at, last_used_at FROM api_tokens
         WHERE user_id = ? AND revoked_at IS NULL ORDER BY created_at",
    )
    .bind(user.id())
    .fetch_all(&state.db.read)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/tokens", tag = "account", summary = "Create an API token (plaintext returned once)", request_body = CreateToken, responses((status = 200, body = CreatedToken), (status = 400, body = crate::error::Problem), (status = 403, description = "Not a browser session", body = crate::error::Problem)))]
pub async fn create_token(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateToken>,
) -> ApiResult<Json<CreatedToken>> {
    user.require_session()?;
    let token_name = name("name", &c.name, 100)?;
    let scope = c.scope.as_deref().unwrap_or("read");
    if scope != "read" && scope != "write" {
        return Err(bad("scope must be read or write"));
    }
    let (token, info) = auth::issue_token(&state, user.id(), &token_name, scope).await?;
    Ok(Json(CreatedToken { token, info }))
}

#[utoipa::path(delete, path = "/tokens/{id}", tag = "account", summary = "Revoke an API token", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn revoke_token(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    user.require_session()?;
    let r = sqlx::query(
        "UPDATE api_tokens SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
    )
    .bind(now())
    .bind(&id)
    .bind(user.id())
    .execute(&state.db.write)
    .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---- users ----------------------------------------------------------------------

/// Everyone on this server (names only), to add to a household or share a list with.
#[utoipa::path(get, path = "/users/directory", tag = "users", summary = "All users' names (for sharing)", responses((status = 200, body = Vec<UserSummary>), (status = 401, body = crate::error::Problem)))]
pub async fn directory(
    State(state): State<AppState>,
    _user: AuthUser,
) -> ApiResult<Json<Vec<UserSummary>>> {
    let rows = sqlx::query_as(
        "SELECT id, username, display_name FROM users WHERE deleted_at IS NULL ORDER BY display_name",
    )
    .fetch_all(&state.db.read)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(get, path = "/users", tag = "users", summary = "List users (admin)", responses((status = 200, body = Vec<Me>), (status = 403, body = crate::error::Problem)))]
pub async fn list_users(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<Me>>> {
    user.require_admin()?;
    let users: Vec<User> =
        sqlx::query_as("SELECT * FROM users WHERE deleted_at IS NULL ORDER BY username")
            .fetch_all(&state.db.read)
            .await?;
    Ok(Json(users.iter().map(Me::from).collect()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateUser {
    username: String,
    password: String,
    display_name: Option<String>,
    #[serde(default)]
    is_admin: bool,
}

#[utoipa::path(post, path = "/users", tag = "users", summary = "Create a user (admin)", request_body = CreateUser, responses((status = 200, body = Me), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 409, description = "Username taken", body = crate::error::Problem)))]
pub async fn create_user(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateUser>,
) -> ApiResult<Json<Me>> {
    user.require_admin()?;
    user.require_session()?;
    let u = auth::create_user(
        &state,
        &c.username,
        &c.password,
        c.display_name.as_deref(),
        c.is_admin,
    )
    .await?;
    Ok(Json(Me::from(&u)))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchUser {
    display_name: Option<String>,
    is_admin: Option<bool>,
    /// Set a new password (signs the user out everywhere and revokes their tokens).
    password: Option<String>,
}

#[utoipa::path(patch, path = "/users/{id}", tag = "users", summary = "Update a user (admin)", params(("id" = String, Path, description = "ULID")), request_body = PatchUser, responses((status = 200, body = Me), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_user(
    State(state): State<AppState>,
    admin: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchUser>,
) -> ApiResult<Json<Me>> {
    admin.require_admin()?;
    admin.require_session()?;
    let mut u = auth::load_user(&state, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if let Some(d) = p.display_name {
        u.display_name = name("display name", &d, 100)?;
    }
    if let Some(a) = p.is_admin {
        if !a && u.id == admin.user.id {
            return Err(bad("you cannot remove your own admin rights"));
        }
        u.is_admin = a;
    }
    let hash = match p.password {
        Some(pw) => {
            auth::validate_new_password(&pw)?;
            Some(auth::hash_password_async(pw).await?)
        }
        None => None,
    };
    let mut tx = state.db.write.begin().await?;
    if let Some(hash) = hash {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(&u.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(&u.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE api_tokens SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL",
        )
        .bind(now())
        .bind(&u.id)
        .execute(&mut *tx)
        .await?;
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "UPDATE users SET display_name = ?, is_admin = ?, updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&u.display_name)
    .bind(u.is_admin)
    .bind(now())
    .bind(rev)
    .bind(&u.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(Me::from(&u)))
}

/// Deactivates the account: it can't sign in and disappears from households and the
/// directory. Their own lists stay in the database (owned data is never silently given away).
#[utoipa::path(delete, path = "/users/{id}", tag = "users", summary = "Deactivate a user (admin)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete_user(
    State(state): State<AppState>,
    admin: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin.require_admin()?;
    admin.require_session()?;
    if id == admin.user.id {
        return Err(bad("you cannot delete yourself"));
    }
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    let r = sqlx::query(
        "UPDATE users SET deleted_at = ?, updated_at = ?, rev = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE api_tokens SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL")
        .bind(&ts)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM household_members WHERE user_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    super::households::ensure_owners(&mut tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
