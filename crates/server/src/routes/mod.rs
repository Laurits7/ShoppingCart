pub mod account;
pub mod households;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
    routing::{delete, get, patch, post},
};
use serde::Serialize;
use tower_http::{
    compression::CompressionLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer,
};

use crate::AppState;

/// The OpenAPI 3.1 description of `/api/v1`, generated from the handlers' annotations.
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "ShoppingCart API",
        description = "Self-hosted shopping lists and wishlists. Authenticate with the session cookie or `Authorization: Bearer <token>` (create tokens in Settings; `read` tokens may only GET). Ids are ULIDs. Errors are RFC 9457 problem documents; things you may not see answer 404.",
    ),
    servers((url = "/api/v1")),
    modifiers(&Security),
    security(("bearer" = []), ("cookie" = [])),
    paths(
        health,
        account::setup_status, account::setup, account::login, account::logout,
        account::me, account::patch_me, account::patch_prefs, account::change_password,
        account::list_sessions, account::revoke_session,
        account::list_tokens, account::create_token, account::revoke_token,
        account::directory, account::list_users, account::create_user, account::patch_user,
        account::delete_user,
        households::list, households::create, households::patch, households::delete,
        households::add_member, households::patch_member, households::remove_member,
    ),
    tags(
        (name = "account", description = "Setup, sign-in, profile, sessions and API tokens"),
        (name = "users", description = "User management (admins) and the user directory"),
        (name = "households", description = "Households: groups of people to share with"),
    )
)]
pub struct ApiDoc;

struct Security;

impl utoipa::Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{
            ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme,
        };
        let c = openapi.components.get_or_insert_with(Default::default);
        c.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
        c.add_security_scheme(
            "cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(
                crate::auth::SESSION_COOKIE,
            ))),
        );
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct Health {
    status: &'static str,
    version: &'static str,
}

#[utoipa::path(get, path = "/health", tag = "account", summary = "Liveness and version (no sign-in needed)", security(()), responses((status = 200, body = Health)))]
pub async fn health() -> axum::Json<Health> {
    axum::Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(<ApiDoc as utoipa::OpenApi>::openapi())
}

/// A small page rendering the OpenAPI document. The viewer comes from a CDN, pinned to one
/// version with an integrity hash; only this page gets the looser CSP it needs.
async fn api_docs() -> impl axum::response::IntoResponse {
    (
        [(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; script-src 'self' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline' https:; font-src 'self' https: data:; img-src 'self' data: https:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'",
        )],
        axum::response::Html(
            r#"<!doctype html><html><head><meta charset="utf-8"><title>ShoppingCart API</title>
<meta name="viewport" content="width=device-width, initial-scale=1"></head><body>
<script id="api-reference" data-url="/api/v1/openapi.json"></script>
<script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.73.0/dist/browser/standalone.js" integrity="sha384-OKyMdsDX84ypSZEhVun8YElXk5c2GQaH3EXPOc6ItmVcLDUAvKHYwvDLvAgsqVtB" crossorigin="anonymous"></script>
</body></html>"#,
        ),
    )
}

/// Only our own scripts run (no inline scripts), no framing, no MIME sniffing, no referrer
/// leaks (SPEC §7.4).
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; manifest-src 'self'; worker-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'";

/// HSTS only when the client really came over HTTPS (it would break plain-HTTP LAN use).
async fn hsts(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let secure = crate::auth::is_secure(&state, req.headers());
    let mut res = next.run(req).await;
    if secure {
        res.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    res
}

/// Largest JSON body the API accepts (picture uploads get their own limit later).
const BODY_LIMIT: usize = 256 * 1024;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/setup", get(account::setup_status).post(account::setup))
        .route("/auth/login", post(account::login))
        .route("/auth/logout", post(account::logout))
        .route("/me", get(account::me).patch(account::patch_me))
        .route("/me/prefs", patch(account::patch_prefs))
        .route("/me/password", post(account::change_password))
        .route("/sessions", get(account::list_sessions))
        .route("/sessions/{id}", delete(account::revoke_session))
        .route(
            "/tokens",
            get(account::list_tokens).post(account::create_token),
        )
        .route("/tokens/{id}", delete(account::revoke_token))
        .route("/users/directory", get(account::directory))
        .route(
            "/users",
            get(account::list_users).post(account::create_user),
        )
        .route(
            "/users/{id}",
            patch(account::patch_user).delete(account::delete_user),
        )
        .route(
            "/households",
            get(households::list).post(households::create),
        )
        .route(
            "/households/{id}",
            patch(households::patch).delete(households::delete),
        )
        .route("/households/{id}/members", post(households::add_member))
        .route(
            "/households/{id}/members/{user_id}",
            patch(households::patch_member).delete(households::remove_member),
        )
        .route("/openapi.json", get(openapi_json))
        .fallback(|| async { crate::error::AppError::NotFound });

    Router::new()
        .nest("/api/v1", api)
        .route("/api/docs", get(api_docs))
        .route("/healthz", get(|| async { "ok" }))
        .fallback(crate::static_files::serve)
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::from_fn_with_state(state.clone(), hsts))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        // A bug that panics fails one request (500) instead of the whole server.
        .layer(tower_http::catch_panic::CatchPanicLayer::new())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
