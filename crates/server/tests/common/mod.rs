//! Test harness shared by the integration tests: a fresh server on a temporary database.

#![allow(dead_code)]

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

pub struct T {
    pub app: Router,
}

pub async fn setup() -> T {
    let dir = std::env::temp_dir().join(format!("shoppingcart-test-{}", ulid::Ulid::new()));
    let (app, _state) = shoppingcart::build(Config::for_tests(dir)).await.unwrap();
    T { app }
}

pub struct Res {
    pub status: StatusCode,
    pub json: Value,
    pub headers: HeaderMap,
    pub cookie: Option<String>,
}

impl T {
    pub async fn send(&self, mut req: Request<Body>) -> Res {
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
    pub async fn req(
        &self,
        method: &str,
        path: &str,
        auth: Option<&str>,
        body: Option<Value>,
    ) -> Res {
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
    pub async fn admin(&self) -> String {
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
    pub async fn user(&self, admin: &str, username: &str) -> (String, String) {
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

    pub async fn login(&self, username: &str, password: &str) -> Option<String> {
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
