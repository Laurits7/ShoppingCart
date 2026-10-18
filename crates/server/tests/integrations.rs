//! Mealie pull and the summary API (Phase 6), against a fake Mealie shaped like its
//! documented API (v3.29).

mod common;

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use common::{T, setup, setup_with};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    items: Vec<Value>,
    /// Make the next PUT (ticking) fail.
    fail_tick: bool,
    ticked: Vec<String>,
}

type Shared = Arc<Mutex<Fake>>;

fn authorized(h: &HeaderMap) -> bool {
    h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer good-token")
}

fn item(id: &str, list: &str, fields: Value) -> Value {
    let mut v = json!({"id": id, "shoppingListId": list, "checked": false, "groupId": "g", "householdId": "h",
                       "quantity": 1, "note": "", "display": "", "position": 0, "food": null, "unit": null, "label": null});
    for (k, x) in fields.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

async fn fake() -> (String, Shared) {
    let shared: Shared = Arc::new(Mutex::new(Fake::default()));
    let app = Router::new()
        .route(
            "/api/households/shopping/lists",
            get(|h: HeaderMap| async move {
                if !authorized(&h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({"detail": "no"})));
                }
                (StatusCode::OK, Json(json!({"items": [{"id": "L1", "name": "Weekly"}, {"id": "L2", "name": "Party"}], "total_pages": 1})))
            }),
        )
        .route(
            "/api/households/shopping/items",
            get(|State(s): State<Shared>, h: HeaderMap| async move {
                if !authorized(&h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({"detail": "no"})));
                }
                // Like a server that ignores the filter: the client must filter too.
                let items = s.lock().unwrap().items.clone();
                (StatusCode::OK, Json(json!({"items": items, "total_pages": 1})))
            })
            .put(|State(s): State<Shared>, h: HeaderMap, Json(body): Json<Vec<Value>>| async move {
                let mut f = s.lock().unwrap();
                if !authorized(&h) || f.fail_tick {
                    f.fail_tick = false;
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
                for b in body {
                    let id = b["id"].as_str().unwrap().to_string();
                    // The whole item comes back, only `checked` changed.
                    if let Some(i) = f.items.iter_mut().find(|i| i["id"] == id.as_str()) {
                        assert_eq!(i["food"], b["food"]);
                        *i = b;
                    }
                    f.ticked.push(id);
                }
                StatusCode::OK
            })
            .delete(|State(s): State<Shared>, Query(q): Query<Vec<(String, String)>>| async move {
                let ids: Vec<String> = q.into_iter().filter(|(k, _)| k == "ids").map(|(_, v)| v).collect();
                s.lock().unwrap().items.retain(|i| !ids.iter().any(|x| i["id"] == x.as_str()));
                StatusCode::OK
            }),
        )
        .with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, shared)
}

async fn shop(t: &T, s: &str) -> (String, String) {
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(s),
            Some(json!({"name": "Rimi"})),
        )
        .await;
    (
        r.json["shop"]["id"].as_str().unwrap().to_string(),
        r.json["shop"]["default_list_id"]
            .as_str()
            .unwrap()
            .to_string(),
    )
}

async fn ok(t: &T, m: &str, path: &str, who: &str, body: Option<Value>) -> Value {
    let r = t.req(m, path, Some(who), body).await;
    assert!(r.status.is_success(), "{m} {path}: {} {}", r.status, r.json);
    r.json
}

#[tokio::test]
async fn mealie_as_an_inbox() {
    let (base, mealie) = fake().await;
    let t = setup().await;
    let s = t.admin().await;
    let (shop_id, list) = shop(&t, &s).await;
    mealie.lock().unwrap().items = vec![
        item(
            "a",
            "L1",
            json!({"food": {"id": "f1", "name": "milk"}, "quantity": 2, "label": {"id": "x", "groupId": "g", "name": "Dairy and eggs"}}),
        ),
        item(
            "b",
            "L1",
            json!({"note": "2 cans tomatoes", "display": "2 cans tomatoes"}),
        ),
        item(
            "c",
            "L1",
            json!({"food": {"id": "f2", "name": "flour", "label": {"id": "y", "groupId": "g", "name": "Baking"}},
                               "quantity": 500, "unit": {"id": "u", "name": "gram", "abbreviation": "g", "useAbbreviation": true}}),
        ),
        item("d", "L1", json!({"note": "already done", "checked": true})),
        item("e", "L2", json!({"note": "from another list"})),
    ];

    // Connecting needs a signed-in browser, not an API token; the token never comes back.
    let tok = ok(
        &t,
        "POST",
        "/api/v1/tokens",
        &s,
        Some(json!({"name": "x", "scope": "write"})),
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string();
    let body = json!({"provider": "mealie", "secret": "good-token",
                      "settings": {"base_url": format!("{base}/"), "target_shop_id": shop_id, "create_categories": true}});
    let r = t
        .req(
            "POST",
            "/api/v1/integrations",
            Some(&tok),
            Some(body.clone()),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let i = ok(&t, "POST", "/api/v1/integrations", &s, Some(body)).await;
    assert_eq!(i["has_secret"], true);
    assert_eq!(i["settings"]["base_url"], base.as_str());
    assert!(!i.to_string().contains("good-token"));
    let id = i["id"].as_str().unwrap().to_string();
    let all = ok(&t, "GET", "/api/v1/integrations", &s, None).await;
    assert!(!all.to_string().contains("good-token"));

    // Pick the Mealie list, switch it on, sync.
    let lists = ok(
        &t,
        "GET",
        &format!("/api/v1/integrations/{id}/mealie/lists"),
        &s,
        None,
    )
    .await;
    assert_eq!(lists[0], json!({"id": "L1", "name": "Weekly"}));
    let mut settings = i["settings"].clone();
    settings["list_id"] = json!("L1");
    ok(
        &t,
        "PATCH",
        &format!("/api/v1/integrations/{id}"),
        &s,
        Some(json!({"settings": settings, "enabled": true})),
    )
    .await;
    let r = ok(
        &t,
        "POST",
        &format!("/api/v1/integrations/{id}/sync"),
        &s,
        None,
    )
    .await;
    assert_eq!(r["imported"], 3, "{r}");
    assert!(r["error"].is_null());
    assert!(r["integration"]["last_sync_at"].is_string());

    let items = ok(&t, "GET", &format!("/api/v1/lists/{list}/items"), &s, None).await;
    let cats = ok(
        &t,
        "GET",
        &format!("/api/v1/lists/{list}/categories"),
        &s,
        None,
    )
    .await;
    let cat = |id: &Value| {
        cats.as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == *id)
            .map(|c| c["name"].as_str().unwrap().to_string())
    };
    let find = |n: &str| {
        items
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == n)
            .unwrap_or_else(|| panic!("no {n} in {items}"))
            .clone()
    };
    let milk = find("milk");
    assert_eq!(
        (
            milk["amount"].as_f64(),
            milk["source"].as_str(),
            milk["external_ref"].as_str()
        ),
        (Some(2.0), Some("mealie"), Some("a"))
    );
    assert_eq!(cat(&milk["category_id"]).as_deref(), Some("Dairy and eggs"));
    let tomatoes = find("tomatoes");
    assert_eq!(
        (tomatoes["amount"].as_f64(), tomatoes["unit"].as_str()),
        (Some(2.0), Some("can"))
    );
    let flour = find("flour");
    assert_eq!(
        (flour["amount"].as_f64(), flour["unit"].as_str()),
        (Some(500.0), Some("g"))
    );
    assert_eq!(
        cat(&flour["category_id"]).as_deref(),
        Some("Baking"),
        "created from the label"
    );
    assert_eq!(items.as_array().unwrap().len(), 3);

    // Ticked in Mealie, so the list there works as an inbox.
    assert_eq!(mealie.lock().unwrap().ticked, vec!["a", "b", "c"]);

    // New in Mealie: comes over; nothing is imported twice, even when ticking failed before.
    {
        let mut f = mealie.lock().unwrap();
        f.items.push(item("f", "L1", json!({"note": "coffee"})));
        f.fail_tick = true;
    }
    let r = ok(
        &t,
        "POST",
        &format!("/api/v1/integrations/{id}/sync"),
        &s,
        None,
    )
    .await;
    assert_eq!(r["imported"], 1, "{r}");
    assert!(
        r["error"].as_str().unwrap().contains("couldn't tick"),
        "{r}"
    );
    assert!(r["integration"]["last_error"].is_string());
    let r = ok(
        &t,
        "POST",
        &format!("/api/v1/integrations/{id}/sync"),
        &s,
        None,
    )
    .await;
    assert_eq!(r["imported"], 0);
    assert!(r["error"].is_null());
    assert!(r["integration"]["last_error"].is_null());
    assert!(mealie.lock().unwrap().ticked.contains(&"f".to_string()));
    let n = ok(&t, "GET", &format!("/api/v1/lists/{list}/items"), &s, None)
        .await
        .as_array()
        .unwrap()
        .len();
    assert_eq!(n, 4);

    // A refused token or an unreachable Mealie only shows up as the last error.
    ok(
        &t,
        "PATCH",
        &format!("/api/v1/integrations/{id}"),
        &s,
        Some(json!({"secret": "wrong"})),
    )
    .await;
    let r = ok(
        &t,
        "POST",
        &format!("/api/v1/integrations/{id}/sync"),
        &s,
        None,
    )
    .await;
    assert!(
        r["error"]
            .as_str()
            .unwrap()
            .contains("refused the API token"),
        "{r}"
    );
    let mut settings = r["integration"]["settings"].clone();
    settings["base_url"] = json!("http://127.0.0.1:9");
    ok(
        &t,
        "PATCH",
        &format!("/api/v1/integrations/{id}"),
        &s,
        Some(json!({"settings": settings, "secret": "good-token"})),
    )
    .await;
    let r = ok(
        &t,
        "POST",
        &format!("/api/v1/integrations/{id}/sync"),
        &s,
        None,
    )
    .await;
    assert!(
        r["error"].as_str().unwrap().contains("couldn't be reached"),
        "{r}"
    );
    let r = t.req("GET", "/api/v1/shopping", Some(&s), None).await;
    assert_eq!(r.status, StatusCode::OK);

    // Settings are checked.
    let mut bad = settings.clone();
    bad["base_url"] = json!("ftp://x");
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/integrations/{id}"),
            Some(&s),
            Some(json!({"settings": bad})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Others see nothing of it.
    let (_, eve) = t.user(&s, "eve").await;
    assert_eq!(
        ok(&t, "GET", "/api/v1/integrations", &eve, None).await,
        json!([])
    );
    for (m, path) in [
        ("PATCH", format!("/api/v1/integrations/{id}")),
        ("DELETE", format!("/api/v1/integrations/{id}")),
        ("POST", format!("/api/v1/integrations/{id}/sync")),
        ("GET", format!("/api/v1/integrations/{id}/mealie/lists")),
    ] {
        let body = (m == "PATCH").then(|| json!({"enabled": false}));
        let r = t.req(m, &path, Some(&eve), body).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{m} {path}");
    }
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/integrations/{id}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        ok(&t, "GET", "/api/v1/integrations", &s, None).await,
        json!([])
    );
}

#[tokio::test]
async fn summary_for_streamline() {
    let t = setup_with(|c| c.public_url = Some("https://cart.example.com".into())).await;
    let s = t.admin().await;
    let (_, list) = shop(&t, &s).await;
    for text in ["milk 2", "flour 1 kg", "bread"] {
        ok(
            &t,
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            &s,
            Some(json!({"text": text})),
        )
        .await;
    }
    let tok = ok(
        &t,
        "POST",
        "/api/v1/tokens",
        &s,
        Some(json!({"name": "streamline", "scope": "read"})),
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string();
    let r = ok(&t, "GET", "/api/v1/summary?limit=2", &tok, None).await;
    let l = &r[0];
    assert_eq!(l["list_id"], list.as_str());
    assert_eq!(l["shop_name"], "Rimi");
    assert_eq!(l["open_count"], 3);
    assert_eq!(l["items"].as_array().unwrap().len(), 2);
    assert_eq!(l["items"][0]["name"], "milk");
    assert_eq!(l["items"][0]["amount"], "2");
    assert_eq!(l["items"][1]["amount"], "1 kg");
    assert_eq!(l["url"], format!("https://cart.example.com/list/{list}"));

    // Another app marks the run done.
    let w = ok(
        &t,
        "POST",
        "/api/v1/tokens",
        &s,
        Some(json!({"name": "w", "scope": "write"})),
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string();
    for i in ok(&t, "GET", &format!("/api/v1/lists/{list}/items"), &s, None)
        .await
        .as_array()
        .unwrap()
    {
        ok(
            &t,
            "PATCH",
            &format!("/api/v1/items/{}", i["id"].as_str().unwrap()),
            &w,
            Some(json!({"checked": true})),
        )
        .await;
    }
    ok(
        &t,
        "POST",
        &format!("/api/v1/lists/{list}/finish"),
        &w,
        None,
    )
    .await;
    let r = ok(&t, "GET", "/api/v1/summary", &tok, None).await;
    assert_eq!(r[0]["open_count"], 0);
    assert_eq!(
        ok(&t, "GET", "/api/v1/summary?non_empty=true", &tok, None).await,
        json!([])
    );

    // Without PUBLIC_URL, links use the address the request came to.
    let t2 = setup().await;
    let s2 = t2.admin().await;
    let (_, list2) = shop(&t2, &s2).await;
    let r = ok(&t2, "GET", "/api/v1/summary", &s2, None).await;
    assert_eq!(r[0]["url"], format!("http://localhost:3100/list/{list2}"));
}
