//! End-to-end API tests against a real (temporary) SQLite database.

use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shoppingcart::config::Config;
use tower::ServiceExt;

struct T {
    app: Router,
}

async fn setup() -> T {
    let dir = std::env::temp_dir().join(format!("shoppingcart-test-{}", ulid::Ulid::new()));
    let (app, _state) = shoppingcart::build(Config::for_tests(dir)).await.unwrap();
    T { app }
}

struct Res {
    status: StatusCode,
    json: Value,
    headers: HeaderMap,
    cookie: Option<String>,
}

impl T {
    async fn send(&self, mut req: Request<Body>) -> Res {
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9999))));
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let cookie = headers
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|c| c.split(';').next())
            .and_then(|kv| kv.strip_prefix("sc_session="))
            .map(String::from);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Res {
            status,
            json,
            headers,
            cookie,
        }
    }

    /// `auth` is a session token, or an API token (starts with `sc_`).
    async fn req(&self, method: &str, path: &str, auth: Option<&str>, body: Option<Value>) -> Res {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "localhost:3100");
        if let Some(a) = auth {
            b = if a.starts_with("sc_") {
                b.header(header::AUTHORIZATION, format!("Bearer {a}"))
            } else {
                b.header(header::COOKIE, format!("sc_session={a}"))
            };
        }
        let body = match body {
            Some(v) => {
                b = b.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        self.send(b.body(body).unwrap()).await
    }

    /// First-run setup; returns the admin's session token.
    async fn admin(&self) -> String {
        let r = self
            .req(
                "POST",
                "/api/v1/setup",
                None,
                Some(json!({"username": "Admin", "password": "password1234"})),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.json);
        r.cookie.unwrap()
    }

    /// Admin creates a user and signs them in; returns (user id, session token).
    async fn user(&self, admin: &str, username: &str) -> (String, String) {
        let r = self
            .req(
                "POST",
                "/api/v1/users",
                Some(admin),
                Some(json!({"username": username, "password": "password1234"})),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.json);
        let id = r.json["id"].as_str().unwrap().to_string();
        (id, self.login(username, "password1234").await.unwrap())
    }

    async fn login(&self, username: &str, password: &str) -> Option<String> {
        self.req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({"username": username, "password": password})),
        )
        .await
        .cookie
    }
}

#[tokio::test]
async fn setup_login_logout() {
    let t = setup().await;
    let r = t.req("GET", "/api/v1/setup", None, None).await;
    assert_eq!(r.json["needs_setup"], true);
    let admin = t.admin().await;
    // Only once.
    let r = t
        .req(
            "POST",
            "/api/v1/setup",
            None,
            Some(json!({"username": "eve", "password": "password1234"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let r = t.req("GET", "/api/v1/me", Some(&admin), None).await;
    assert_eq!(
        r.json["username"], "admin",
        "usernames are stored lower-case"
    );
    assert_eq!(r.json["is_admin"], true);
    assert!(r.json.get("password_hash").is_none());

    // Case-insensitive username, wrong password.
    assert!(t.login("ADMIN", "password1234").await.is_some());
    assert!(t.login("admin", "wrong-password").await.is_none());
    assert!(t.login("nobody", "password1234").await.is_none());

    let r = t
        .req("POST", "/api/v1/auth/logout", Some(&admin), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t.req("GET", "/api/v1/me", Some(&admin), None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn short_passwords_are_rejected() {
    let t = setup().await;
    let r = t
        .req(
            "POST",
            "/api/v1/setup",
            None,
            Some(json!({"username": "admin", "password": "short"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
}

#[tokio::test]
async fn login_is_throttled() {
    let t = setup().await;
    t.admin().await;
    for _ in 0..10 {
        assert!(t.login("admin", "wrong-password").await.is_none());
    }
    let r = t
        .req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "admin", "password": "password1234"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn cross_origin_writes_are_rejected() {
    let t = setup().await;
    let admin = t.admin().await;
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/me")
        .header(header::HOST, "localhost:3100")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::COOKIE, format!("sc_session={admin}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"display_name": "pwned"}).to_string()))
        .unwrap();
    assert_eq!(t.send(req).await.status, StatusCode::FORBIDDEN);
    // Same origin is fine.
    let req = Request::builder()
        .method("PATCH")
        .uri("/api/v1/me")
        .header(header::HOST, "localhost:3100")
        .header(header::ORIGIN, "http://localhost:3100")
        .header(header::COOKIE, format!("sc_session={admin}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"display_name": "Boss"}).to_string()))
        .unwrap();
    let r = t.send(req).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["display_name"], "Boss");
}

#[tokio::test]
async fn api_tokens_and_scopes() {
    let t = setup().await;
    let admin = t.admin().await;
    let r = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&admin),
            Some(json!({"name": "Streamline"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["info"]["scope"], "read", "read is the default scope");
    let read = r.json["token"].as_str().unwrap().to_string();
    assert!(read.starts_with("sc_"));
    let r = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&admin),
            Some(json!({"name": "HA", "scope": "write"})),
        )
        .await;
    let write = r.json["token"].as_str().unwrap().to_string();

    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&read), None).await.status,
        StatusCode::OK
    );
    let r = t
        .req(
            "PATCH",
            "/api/v1/me",
            Some(&read),
            Some(json!({"display_name": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "read tokens can't write");
    let r = t
        .req(
            "PATCH",
            "/api/v1/me",
            Some(&write),
            Some(json!({"display_name": "Y"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);

    // Tokens can't manage credentials.
    let r = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&write),
            Some(json!({"name": "more"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        t.req("GET", "/api/v1/sessions", Some(&write), None)
            .await
            .status,
        StatusCode::FORBIDDEN
    );

    // Listing never shows the secret; revoking works.
    let r = t.req("GET", "/api/v1/tokens", Some(&admin), None).await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    assert!(!r.json.to_string().contains(&read));
    let id = r.json[0]["id"].as_str().unwrap();
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/tokens/{id}"),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&read), None).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn password_change_ends_other_access() {
    let t = setup().await;
    let admin = t.admin().await;
    let other = t.login("admin", "password1234").await.unwrap();
    let r = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&admin),
            Some(json!({"name": "t"})),
        )
        .await;
    let token = r.json["token"].as_str().unwrap().to_string();

    let r = t
        .req(
            "POST",
            "/api/v1/me/password",
            Some(&admin),
            Some(json!({"current_password": "nope-nope-nope", "new_password": "new-password-1"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = t
        .req(
            "POST",
            "/api/v1/me/password",
            Some(&admin),
            Some(json!({"current_password": "password1234", "new_password": "new-password-1"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&admin), None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&other), None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&token), None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert!(t.login("admin", "new-password-1").await.is_some());
}

#[tokio::test]
async fn sessions_list_and_revoke() {
    let t = setup().await;
    let admin = t.admin().await;
    let phone = t.login("admin", "password1234").await.unwrap();
    let r = t.req("GET", "/api/v1/sessions", Some(&admin), None).await;
    let list = r.json.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list.iter().filter(|s| s["current"] == true).count(), 1);
    let other = list.iter().find(|s| s["current"] == false).unwrap();
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/sessions/{}", other["id"].as_str().unwrap()),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&phone), None).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn user_management_is_admin_only() {
    let t = setup().await;
    let admin = t.admin().await;
    let (bob_id, bob) = t.user(&admin, "bob").await;
    assert_eq!(
        t.req("GET", "/api/v1/users", Some(&bob), None).await.status,
        StatusCode::FORBIDDEN
    );
    let r = t
        .req(
            "POST",
            "/api/v1/users",
            Some(&bob),
            Some(json!({"username": "eve", "password": "password1234"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Duplicate usernames, case-insensitively.
    let r = t
        .req(
            "POST",
            "/api/v1/users",
            Some(&admin),
            Some(json!({"username": "BOB", "password": "password1234"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    // Everyone sees the directory (names only).
    let r = t
        .req("GET", "/api/v1/users/directory", Some(&bob), None)
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    assert!(r.json[0].get("is_admin").is_none());

    // Admin resets bob's password: bob is signed out.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/users/{bob_id}"),
            Some(&admin),
            Some(json!({"password": "reset-password-1"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&bob), None).await.status,
        StatusCode::UNAUTHORIZED
    );
    let bob = t.login("bob", "reset-password-1").await.unwrap();

    // Deactivate.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/users/{bob_id}"),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        t.req("GET", "/api/v1/me", Some(&bob), None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert!(t.login("bob", "reset-password-1").await.is_none());
}

#[tokio::test]
async fn households() {
    let t = setup().await;
    let admin = t.admin().await;
    let (bob_id, bob) = t.user(&admin, "bob").await;
    let (carol_id, carol) = t.user(&admin, "carol").await;

    let r = t
        .req(
            "POST",
            "/api/v1/households",
            Some(&bob),
            Some(json!({"name": " Home "})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["name"], "Home");
    assert_eq!(r.json["members"][0]["role"], "owner");
    let hid = r.json["id"].as_str().unwrap().to_string();
    let path = format!("/api/v1/households/{hid}");

    // Carol can't see it (404, not 403) and can't add herself.
    let r = t
        .req("PATCH", &path, Some(&carol), Some(json!({"name": "Mine"})))
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = t
        .req(
            "POST",
            &format!("{path}/members"),
            Some(&carol),
            Some(json!({"user_id": carol_id})),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(
        t.req("GET", "/api/v1/households", Some(&carol), None)
            .await
            .json,
        json!([])
    );

    // Bob adds Carol as a member; she sees it but can't manage it.
    let r = t
        .req(
            "POST",
            &format!("{path}/members"),
            Some(&bob),
            Some(json!({"user_id": carol_id})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["members"].as_array().unwrap().len(), 2);
    let r = t.req("GET", "/api/v1/households", Some(&carol), None).await;
    assert_eq!(r.json[0]["id"], hid.as_str());
    let r = t
        .req("PATCH", &path, Some(&carol), Some(json!({"name": "Mine"})))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // The last owner can't step down; but if he leaves, Carol becomes owner.
    let r = t
        .req(
            "PATCH",
            &format!("{path}/members/{bob_id}"),
            Some(&bob),
            Some(json!({"role": "member"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t
        .req(
            "DELETE",
            &format!("{path}/members/{bob_id}"),
            Some(&bob),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t.req("GET", "/api/v1/households", Some(&carol), None).await;
    assert_eq!(r.json[0]["members"][0]["role"], "owner");
    assert_eq!(
        t.req("GET", "/api/v1/households", Some(&bob), None)
            .await
            .json,
        json!([])
    );

    // Admins see and manage every household.
    let r = t.req("GET", "/api/v1/households", Some(&admin), None).await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);
    let r = t.req("DELETE", &path, Some(&admin), None).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        t.req("GET", "/api/v1/households", Some(&carol), None)
            .await
            .json,
        json!([])
    );
}

#[tokio::test]
async fn household_create_is_idempotent() {
    let t = setup().await;
    let admin = t.admin().await;
    let id = ulid::Ulid::new().to_string();
    for _ in 0..2 {
        let r = t
            .req(
                "POST",
                "/api/v1/households",
                Some(&admin),
                Some(json!({"id": id, "name": "Home"})),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK);
    }
    let r = t.req("GET", "/api/v1/households", Some(&admin), None).await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn headers_health_and_fallbacks() {
    let t = setup().await;
    let r = t.req("GET", "/api/v1/health", None, None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["status"], "ok");
    let csp = r.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("script-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(r.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(
        r.headers.get(header::STRICT_TRANSPORT_SECURITY).is_none(),
        "no HSTS over plain HTTP"
    );

    let r = t.req("GET", "/api/v1/nope", None, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(
        t.req("GET", "/api/v1/me", None, None).await.status,
        StatusCode::UNAUTHORIZED
    );

    let r = t.req("GET", "/api/v1/openapi.json", None, None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.json["paths"]["/households"].is_object());
}
