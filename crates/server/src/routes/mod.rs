pub mod account;
pub mod admin;
pub mod catalog;
pub mod households;
pub mod images;
pub mod integrations;
pub mod items;
pub mod lookup;
pub mod products;
pub mod shares;
pub mod shops;
pub mod summary;
pub mod sync;
pub mod wishes;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
    routing::{delete, get, patch, post, put},
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
        sync::shopping, sync::sync, sync::ticket, sync::events, summary::summary,
        admin::backups, admin::backup_now, admin::download, admin::export,
        integrations::list, integrations::create, integrations::patch, integrations::delete,
        integrations::sync_now, integrations::mealie_lists,
        shares::list, shares::create, shares::patch, shares::delete,
        shares::list_wishlist, shares::create_wishlist,
        wishes::list_persons, wishes::create_person, wishes::patch_person, wishes::delete_person,
        wishes::list, wishes::create, wishes::patch, wishes::delete, wishes::move_wishlist,
        wishes::create_section, wishes::patch_section, wishes::delete_section, wishes::move_section,
        wishes::create_item, wishes::patch_item, wishes::delete_item, wishes::restore_items, wishes::move_item,
        wishes::claim, wishes::patch_claim, wishes::release,
        shops::list_shops, shops::create_shop, shops::patch_shop, shops::delete_shop,
        shops::list_lists, shops::get_list, shops::create_list, shops::patch_list, shops::delete_list,
        items::get_categories, items::create_category, items::patch_category, items::delete_category,
        items::get_items, items::create_item, items::create_inbox_item, items::patch_item,
        items::step_item, items::delete_item, items::restore_items, items::finish, items::add_from_template,
        shops::move_shop, shops::move_list, items::move_category, items::move_item, items::move_items,
        images::upload, images::get, lookup::search, lookup::barcode, lookup::picture, lookup::import,
        products::list, products::create, products::patch, products::delete, products::set_preferred,
        catalog::search, catalog::frequent, catalog::patch, catalog::delete, catalog::merge,
    ),
    tags(
        (name = "account", description = "Setup, sign-in, profile, sessions and API tokens"),
        (name = "users", description = "User management (admins) and the user directory"),
        (name = "households", description = "Households: groups of people to share with"),
        (name = "shopping", description = "Shops, lists, templates, the inbox, categories and items"),
        (name = "catalog", description = "Your item catalogue: everything you've added before"),
        (name = "products", description = "Brands, pictures and product lookups"),
        (name = "sharing", description = "Sharing lists and wishlists with people and households"),
        (name = "wishlists", description = "Wishlists, gift lists, persons and claims"),
        (name = "integrations", description = "Mealie, Streamline and other apps"),
        (name = "admin", description = "Backups (admins)"),
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
        .route("/shopping", get(sync::shopping))
        .route("/sync", get(sync::sync))
        .route("/summary", get(summary::summary))
        .route("/export", get(admin::export))
        .route(
            "/admin/backups",
            get(admin::backups).post(admin::backup_now),
        )
        .route("/admin/backups/{name}", get(admin::download))
        .route(
            "/integrations",
            get(integrations::list).post(integrations::create),
        )
        .route(
            "/integrations/{id}",
            patch(integrations::patch).delete(integrations::delete),
        )
        .route("/integrations/{id}/sync", post(integrations::sync_now))
        .route(
            "/integrations/{id}/mealie/lists",
            get(integrations::mealie_lists),
        )
        .route("/events", get(sync::events))
        .route("/events/ticket", post(sync::ticket))
        .route("/lists/{id}/shares", get(shares::list).post(shares::create))
        .route("/shares/{id}", patch(shares::patch).delete(shares::delete))
        .route(
            "/wishlists/{id}/shares",
            get(shares::list_wishlist).post(shares::create_wishlist),
        )
        .route(
            "/persons",
            get(wishes::list_persons).post(wishes::create_person),
        )
        .route(
            "/persons/{id}",
            patch(wishes::patch_person).delete(wishes::delete_person),
        )
        .route("/wishlists", get(wishes::list).post(wishes::create))
        .route(
            "/wishlists/{id}",
            patch(wishes::patch).delete(wishes::delete),
        )
        .route("/wishlists/{id}/move", post(wishes::move_wishlist))
        .route("/wishlists/{id}/sections", post(wishes::create_section))
        .route("/wishlists/{id}/items", post(wishes::create_item))
        .route(
            "/wish-sections/{id}",
            patch(wishes::patch_section).delete(wishes::delete_section),
        )
        .route("/wish-sections/{id}/move", post(wishes::move_section))
        .route("/wish-items/restore", post(wishes::restore_items))
        .route(
            "/wish-items/{id}",
            patch(wishes::patch_item).delete(wishes::delete_item),
        )
        .route("/wish-items/{id}/move", post(wishes::move_item))
        .route("/wish-items/{id}/claim", post(wishes::claim))
        .route(
            "/claims/{id}",
            patch(wishes::patch_claim).delete(wishes::release),
        )
        .route("/shops", get(shops::list_shops).post(shops::create_shop))
        .route(
            "/shops/{id}",
            patch(shops::patch_shop).delete(shops::delete_shop),
        )
        .route("/inbox/items", post(items::create_inbox_item))
        .route("/shops/{id}/move", post(shops::move_shop))
        .route("/lists/{id}/move", post(shops::move_list))
        .route("/categories/{id}/move", post(items::move_category))
        .route("/items/move", post(items::move_items))
        .route("/items/{id}/move", post(items::move_item))
        .route("/lists", get(shops::list_lists).post(shops::create_list))
        .route(
            "/lists/{id}",
            get(shops::get_list)
                .patch(shops::patch_list)
                .delete(shops::delete_list),
        )
        .route(
            "/lists/{id}/categories",
            get(items::get_categories).post(items::create_category),
        )
        .route(
            "/lists/{id}/items",
            get(items::get_items).post(items::create_item),
        )
        .route("/lists/{id}/finish", post(items::finish))
        .route("/lists/{id}/from-template", post(items::add_from_template))
        .route(
            "/categories/{id}",
            patch(items::patch_category).delete(items::delete_category),
        )
        .route("/items/restore", post(items::restore_items))
        .route(
            "/items/{id}",
            patch(items::patch_item).delete(items::delete_item),
        )
        .route("/items/{id}/step", post(items::step_item))
        .route("/catalog", get(catalog::search))
        .route("/catalog/frequent", get(catalog::frequent))
        .route("/catalog/merge", post(catalog::merge))
        .route(
            "/catalog/{id}",
            patch(catalog::patch).delete(catalog::delete),
        )
        .route(
            "/images",
            post(images::upload).layer(axum::extract::DefaultBodyLimit::max(
                state.config.max_upload_bytes,
            )),
        )
        .route("/images/{id}", get(images::get))
        .route("/lookup", get(lookup::search))
        .route("/lookup/barcode/{code}", get(lookup::barcode))
        .route("/lookup/picture/{code}", get(lookup::picture))
        .route("/lookup/{code}/import", post(lookup::import))
        .route(
            "/catalog/{id}/products",
            get(products::list).post(products::create),
        )
        .route("/catalog/{id}/preferred", put(products::set_preferred))
        .route(
            "/products/{id}",
            patch(products::patch).delete(products::delete),
        )
        .route("/openapi.json", get(openapi_json))
        .fallback(|| async { crate::error::AppError::NotFound });

    Router::new()
        .nest("/api/v1", api)
        .route("/api/docs", get(api_docs))
        .route("/healthz", get(|| async { "ok" }))
        .fallback(crate::static_files::serve)
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::events::publish_writes,
        ))
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
