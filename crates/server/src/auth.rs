//! Password hashing, sessions (cookie) and API tokens (bearer), and the `AuthUser`
//! extractor used by every protected route (SPEC §7.1, §7.2).

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, Method, header, request::Parts},
};
use shoppingcart_domain::account;

use crate::{
    AppState,
    config::CookieSecure,
    error::{AppError, bad},
    models::User,
    util::{new_id, now, random_token, sha256_hex},
};

pub const SESSION_COOKIE: &str = "sc_session";
pub const TOKEN_PREFIX: &str = "sc_";
/// Failed sign-ins allowed per username, and per client address, within the window.
const MAX_FAILURES: u32 = 10;
const MAX_FAILURES_PER_IP: u32 = 50;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
/// Password hashing takes ~19 MiB and real CPU time: at most this many at once.
static HASHING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
/// Sliding sessions are extended at most this often, so reads don't turn into writes.
const SESSION_TOUCH_SECS: i64 = 3600;

/// argon2id with OWASP's recommended minimum (19 MiB, 2 passes, 1 lane).
fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19 * 1024, 2, 1, None).expect("valid argon2 params"),
    )
}

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash failed: {e}"))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| argon2().verify_password(pw.as_bytes(), &h).is_ok())
}

/// Hashing is deliberately slow; keep it off the async worker threads.
pub async fn hash_password_async(pw: String) -> anyhow::Result<String> {
    let _permit = HASHING.acquire().await?;
    tokio::task::spawn_blocking(move || hash_password(&pw)).await?
}

pub async fn verify_password_async(pw: String, hash: Option<String>) -> bool {
    // Verify against a dummy hash for unknown users so timing doesn't reveal usernames.
    static DUMMY: OnceLock<String> = OnceLock::new();
    if pw.len() > account::MAX_PASSWORD_BYTES {
        return false;
    }
    let Ok(_permit) = HASHING.acquire().await else {
        return false;
    };
    tokio::task::spawn_blocking(move || {
        let hash = hash.unwrap_or_else(|| {
            DUMMY
                .get_or_init(|| hash_password("dummy-password").unwrap())
                .clone()
        });
        verify_password(&pw, &hash)
    })
    .await
    .unwrap_or(false)
}

pub fn validate_new_password(pw: &str) -> Result<(), AppError> {
    account::check_password(pw).map_err(bad)
}

pub async fn create_user(
    state: &AppState,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    is_admin: bool,
) -> Result<User, AppError> {
    let username = account::normalize_username(username);
    account::check_username(&username).map_err(bad)?;
    validate_new_password(password)?;
    let display_name = match display_name.filter(|d| !d.trim().is_empty()) {
        Some(d) => crate::util::name("display name", d, 100)?,
        None => username.clone(),
    };
    let hash = hash_password_async(password.to_string()).await?;
    let mut tx = state.db.write.begin().await?;
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_some() {
        return Err(AppError::Conflict("username already taken".into()));
    }
    let rev = crate::db::next_rev(&mut tx).await?;
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO users (id, username, display_name, password_hash, is_admin, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(&display_name)
    .bind(&hash)
    .bind(is_admin)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    load_user(state, &id).await?.ok_or(AppError::NotFound)
}

pub async fn load_user(state: &AppState, id: &str) -> sqlx::Result<Option<User>> {
    sqlx::query_as("SELECT * FROM users WHERE id = ? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&state.db.read)
        .await
}

pub async fn user_count(state: &AppState) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
        .fetch_one(&state.db.read)
        .await
}

/// Create the first admin from `INITIAL_ADMIN_USER`/`INITIAL_ADMIN_PASSWORD` if there are
/// no users yet. Without them, the web UI offers a first-run setup.
pub async fn bootstrap_admin(state: &AppState) -> anyhow::Result<()> {
    if user_count(state).await? > 0 {
        return Ok(());
    }
    match (
        &state.config.initial_admin_user,
        &state.config.initial_admin_password,
    ) {
        (Some(u), Some(p)) => {
            create_user(state, u, p, None, true)
                .await
                .map_err(|e| anyhow::anyhow!("creating initial admin: {e:?}"))?;
            tracing::info!("created initial admin user {u:?}");
        }
        _ => tracing::info!("no users yet: open the web UI to create the admin account"),
    }
    Ok(())
}

/// A new API token for `user_id`; the plaintext is returned once, only its hash is kept.
pub async fn issue_token(
    state: &AppState,
    user_id: &str,
    name: &str,
    scope: &str,
) -> Result<(String, crate::models::ApiToken), AppError> {
    let token = format!("{TOKEN_PREFIX}{}", random_token());
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, scope, created_at) VALUES (?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(sha256_hex(&token))
    .bind(scope)
    .bind(&ts)
    .execute(&state.db.write)
    .await?;
    let info = crate::models::ApiToken {
        id,
        name: name.into(),
        scope: scope.into(),
        created_at: ts,
        last_used_at: None,
    };
    Ok((token, info))
}

// ---- sessions ---------------------------------------------------------------

fn session_expiry(state: &AppState) -> String {
    (chrono::Utc::now() + chrono::Duration::days(state.config.session_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub async fn create_session(
    state: &AppState,
    user_id: &str,
    user_agent: Option<&str>,
) -> Result<String, AppError> {
    let token = random_token();
    let ts = now();
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, created_at, last_seen_at, expires_at, user_agent) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(new_id())
    .bind(sha256_hex(&token))
    .bind(user_id)
    .bind(&ts)
    .bind(&ts)
    .bind(session_expiry(state))
    .bind(user_agent.map(|s| s.chars().take(200).collect::<String>()))
    .execute(&state.db.write)
    .await?;
    Ok(token)
}

pub fn session_cookie(state: &AppState, headers: &HeaderMap, token: &str) -> String {
    let max_age = state.config.session_days * 86400;
    let secure = if is_secure(state, headers) {
        "; Secure"
    } else {
        ""
    };
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub fn clear_cookie(state: &AppState, headers: &HeaderMap) -> String {
    let secure = if is_secure(state, headers) {
        "; Secure"
    } else {
        ""
    };
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

/// Whether the client reached us over HTTPS. We never terminate TLS ourselves, so this is
/// only known through a trusted reverse proxy, or forced by `COOKIE_SECURE`.
pub fn is_secure(state: &AppState, headers: &HeaderMap) -> bool {
    match state.config.cookie_secure {
        CookieSecure::Always => true,
        CookieSecure::Never => false,
        CookieSecure::Auto => {
            state.config.trust_proxy
                && headers
                    .get("x-forwarded-proto")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| {
                        v.split(',')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .eq_ignore_ascii_case("https")
                    })
        }
    }
}

pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

// ---- login throttling ---------------------------------------------------------

/// Refuse when a key is over its limit; otherwise count this attempt up front (so a burst
/// of parallel requests can't all pass before any failure is recorded). A successful
/// sign-in clears the username's count again.
pub async fn check_throttle(state: &AppState, keys: &[String]) -> Result<(), AppError> {
    let mut map = state.login_failures.lock().await;
    map.retain(|_, (_, start)| start.elapsed() < FAILURE_WINDOW);
    let limit = |k: &str| {
        if k.starts_with("ip:") {
            MAX_FAILURES_PER_IP
        } else {
            MAX_FAILURES
        }
    };
    if keys
        .iter()
        .any(|k| map.get(k).is_some_and(|(n, _)| *n >= limit(k)))
    {
        return Err(AppError::TooManyRequests);
    }
    for k in keys {
        map.entry(k.clone()).or_insert((0, Instant::now())).0 += 1;
    }
    Ok(())
}

pub async fn clear_failures(state: &AppState, keys: &[String]) {
    let mut map = state.login_failures.lock().await;
    for k in keys {
        map.remove(k);
    }
}

/// The client's address: the TCP peer, or behind a trusted proxy the address the proxy
/// appended to `X-Forwarded-For` (the right-most entry, which clients can't forge).
pub fn client_ip(state: &AppState, headers: &HeaderMap, peer: std::net::IpAddr) -> String {
    if state.config.trust_proxy
        && let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| v.parse::<std::net::IpAddr>().is_ok())
    {
        return ip;
    }
    peer.to_string()
}

// ---- extractor ------------------------------------------------------------------

/// How the request was authenticated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// Browser session (hash of the cookie token).
    Session(String),
    /// API token (hash) and whether it may write.
    Token { hash: String, write: bool },
}

/// The authenticated user, from a bearer API token or the session cookie.
pub struct AuthUser {
    pub user: User,
    pub credential: Credential,
}

impl AuthUser {
    pub fn id(&self) -> &str {
        &self.user.id
    }

    pub fn session_hash(&self) -> Option<&str> {
        match &self.credential {
            Credential::Session(h) => Some(h),
            Credential::Token { .. } => None,
        }
    }

    /// Account security (password, tokens, sessions) is only managed from a signed-in
    /// browser, never with an API token.
    pub fn require_session(&self) -> Result<(), AppError> {
        match self.credential {
            Credential::Session(_) => Ok(()),
            Credential::Token { .. } => Err(AppError::Forbidden(
                "this needs a signed-in browser session, not an API token".into(),
            )),
        }
    }

    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.user.is_admin {
            Ok(())
        } else {
            Err(AppError::Forbidden("admin only".into()))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;
        let writes = !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
        if let Some(token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        {
            let hash = sha256_hex(token.trim());
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT user_id, scope FROM api_tokens WHERE token_hash = ? AND revoked_at IS NULL",
            )
            .bind(&hash)
            .fetch_optional(&state.db.read)
            .await?;
            let (user_id, scope) = row.ok_or(AppError::Unauthorized)?;
            let user = load_user(state, &user_id)
                .await?
                .ok_or(AppError::Unauthorized)?;
            let write = scope == "write";
            if writes && !write {
                return Err(AppError::Forbidden("this API token is read-only".into()));
            }
            let _ = sqlx::query("UPDATE api_tokens SET last_used_at = ? WHERE token_hash = ?")
                .bind(now())
                .bind(&hash)
                .execute(&state.db.write)
                .await;
            return Ok(AuthUser {
                user,
                credential: Credential::Token { hash, write },
            });
        }

        let token = cookie_value(headers, SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
        let hash = sha256_hex(token);
        let ts = now();
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT user_id, last_seen_at FROM sessions WHERE token_hash = ? AND expires_at > ?",
        )
        .bind(&hash)
        .bind(&ts)
        .fetch_optional(&state.db.read)
        .await?;
        let (user_id, last_seen) = row.ok_or(AppError::Unauthorized)?;
        let user = load_user(state, &user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        // CSRF defence for cookie auth: state-changing requests must come from our origin.
        if writes {
            check_origin(state, headers)?;
        }
        let stale = chrono::DateTime::parse_from_rfc3339(&last_seen)
            .map(|t| (chrono::Utc::now() - t.to_utc()).num_seconds() > SESSION_TOUCH_SECS)
            .unwrap_or(true);
        if stale {
            let _ = sqlx::query(
                "UPDATE sessions SET last_seen_at = ?, expires_at = ? WHERE token_hash = ?",
            )
            .bind(&ts)
            .bind(session_expiry(state))
            .bind(&hash)
            .execute(&state.db.write)
            .await;
        }
        Ok(AuthUser {
            user,
            credential: Credential::Session(hash),
        })
    }
}

/// Reject cross-origin browser requests. Browsers always send `Origin` on cross-origin and
/// non-GET fetches; if it is missing, the request is not from a page on another site.
pub fn check_origin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return Ok(());
    };
    let origin_host = origin.split_once("://").map(|(_, h)| h).unwrap_or(origin);
    let header_host = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or("").trim())
    };
    let mut allowed = vec![];
    if let Some(h) = header_host(header::HOST.as_str()) {
        allowed.push(h);
    }
    if state.config.trust_proxy
        && let Some(h) = header_host("x-forwarded-host")
    {
        allowed.push(h);
    }
    if allowed.iter().any(|h| h.eq_ignore_ascii_case(origin_host)) {
        Ok(())
    } else {
        Err(AppError::Forbidden("cross-origin request rejected".into()))
    }
}
