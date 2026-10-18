//! Sharing lists, /sync and live events (Phase 4).

mod common;

use std::{net::SocketAddr, time::Duration};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header},
};
use common::{T, setup};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

struct World {
    t: T,
    ann: String,
    bob: (String, String),
    eve: (String, String),
    shop: String,
    list: String,
    milk: String,
}

/// Ann (admin) has a shop with a list and milk on it; Bob and Eve are other users.
async fn world() -> World {
    let t = setup().await;
    let ann = t.admin().await;
    let bob = t.user(&ann, "bob").await;
    let eve = t.user(&ann, "eve").await;
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&ann),
            Some(json!({"name": "Rimi"})),
        )
        .await;
    let shop = r.json["shop"]["id"].as_str().unwrap().to_string();
    let list = r.json["shop"]["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            Some(&ann),
            Some(json!({"text": "milk 2"})),
        )
        .await;
    let milk = r.json["item"]["id"].as_str().unwrap().to_string();
    World {
        t,
        ann,
        bob,
        eve,
        shop,
        list,
        milk,
    }
}

async fn share(w: &World, principal_type: &str, principal_id: &str, role: &str) -> Value {
    let r = w
        .t
        .req(
            "POST",
            &format!("/api/v1/lists/{}/shares", w.list),
            Some(&w.ann),
            Some(json!({"principal_type": principal_type, "principal_id": principal_id, "role": role})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    r.json
}

async fn sees(w: &World, who: &str) -> bool {
    let r = w.t.req("GET", "/api/v1/shopping", Some(who), None).await;
    r.json["lists"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["id"] == w.list.as_str())
}

#[tokio::test]
async fn viewers_read_editors_edit_owners_manage() {
    let w = world().await;
    let (t, list, milk) = (&w.t, &w.list, &w.milk);
    let (bob_id, bob) = &w.bob;
    let (eve_id, eve) = &w.eve;
    share(&w, "user", bob_id, "viewer").await;
    share(&w, "user", eve_id, "editor").await;

    // Both see the list, under Ann's shop name, with their role.
    let data = t.req("GET", "/api/v1/shopping", Some(bob), None).await.json;
    assert_eq!(data["lists"][0]["role"], "viewer");
    assert_eq!(data["lists"][0]["shop_name"], "Rimi");
    assert_eq!(data["lists"][0]["owner_name"], "admin");
    assert_eq!(data["items"][0]["name"], "milk");
    assert_eq!(data["shops"], json!([]), "shops stay personal");
    let r = t
        .req("GET", &format!("/api/v1/lists/{list}"), Some(eve), None)
        .await;
    assert_eq!(r.json["role"], "editor");

    let cat = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(bob),
            None,
        )
        .await
        .json[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Item-level changes: editors yes, viewers no (403: they can see it).
    let edits: Vec<(&str, String, Option<Value>)> = vec![
        (
            "POST",
            format!("/api/v1/lists/{list}/items"),
            Some(json!({"name": "bread"})),
        ),
        (
            "PATCH",
            format!("/api/v1/items/{milk}"),
            Some(json!({"checked": true})),
        ),
        (
            "POST",
            format!("/api/v1/items/{milk}/step"),
            Some(json!({"by": 1})),
        ),
        (
            "POST",
            format!("/api/v1/items/{milk}/move"),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/lists/{list}/categories"),
            Some(json!({"name": "Snacks"})),
        ),
        (
            "PATCH",
            format!("/api/v1/categories/{cat}"),
            Some(json!({"name": "Fruit"})),
        ),
        (
            "POST",
            format!("/api/v1/categories/{cat}/move"),
            Some(json!({})),
        ),
        ("POST", format!("/api/v1/lists/{list}/finish"), None),
    ];
    for (m, path, body) in &edits {
        let r = t.req(m, path, Some(bob), body.clone()).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "viewer {m} {path}");
        let r = t.req(m, path, Some(eve), body.clone()).await;
        assert!(r.status.is_success(), "editor {m} {path}: {}", r.json);
    }
    // What Eve adds goes into her own catalogue, not Ann's.
    let mine = t.req("GET", "/api/v1/catalog", Some(eve), None).await.json;
    assert!(
        mine.as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "bread")
    );
    let anns = t
        .req("GET", "/api/v1/catalog", Some(&w.ann), None)
        .await
        .json;
    assert!(
        !anns
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "bread")
    );

    // Managing the list is the owner's: 403 for both.
    let manage: Vec<(&str, String, Option<Value>)> = vec![
        (
            "PATCH",
            format!("/api/v1/lists/{list}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/lists/{list}"), None),
        (
            "POST",
            format!("/api/v1/lists/{list}/move"),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/lists/{list}/shares"),
            Some(json!({"principal_type": "user", "principal_id": bob_id, "role": "editor"})),
        ),
    ];
    for (m, path, body) in &manage {
        for who in [bob, eve] {
            let r = t.req(m, path, Some(who), body.clone()).await;
            assert_eq!(r.status, StatusCode::FORBIDDEN, "{m} {path}");
        }
    }
    // The shop and its staples stay Ann's.
    let shop = &w.shop;
    let r = t
        .req(
            "GET",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(eve),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = t
        .req(
            "POST",
            &format!("/api/v1/shops/{shop}/items"),
            Some(eve),
            Some(json!({"name": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // Shares: everyone with access sees who else has it; only Ann changes them.
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/shares"),
            Some(bob),
            None,
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    let eve_share = r
        .json
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["principal_id"] == eve_id.as_str())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/shares/{eve_share}"),
            Some(eve),
            Some(json!({"role": "editor"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/shares/{eve_share}"),
            Some(&w.ann),
            Some(json!({"role": "viewer"})),
        )
        .await;
    assert_eq!(r.json["role"], "viewer");
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            Some(eve),
            Some(json!({"name": "eggs"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "a viewer now");

    // Sharing again with the same person changes the role rather than adding a share.
    share(&w, "user", eve_id, "editor").await;
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/shares"),
            Some(&w.ann),
            None,
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn households_now_and_later() {
    let w = world().await;
    let t = &w.t;
    let (bob_id, bob) = &w.bob;
    let (eve_id, eve) = &w.eve;
    let h = t
        .req(
            "POST",
            "/api/v1/households",
            Some(&w.ann),
            Some(json!({"name": "Home"})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    t.req(
        "POST",
        &format!("/api/v1/households/{h}/members"),
        Some(&w.ann),
        Some(json!({"user_id": bob_id})),
    )
    .await;
    let s = share(&w, "household", &h, "editor").await;
    assert_eq!(s["name"], "Home");
    assert_eq!(s["members"], json!(["admin", "bob"]));
    assert!(sees(&w, bob).await);
    assert!(!sees(&w, eve).await);
    // Future members too.
    t.req(
        "POST",
        &format!("/api/v1/households/{h}/members"),
        Some(&w.ann),
        Some(json!({"user_id": eve_id})),
    )
    .await;
    assert!(sees(&w, eve).await);
    // Leaving the household ends it at once.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/households/{h}/members/{eve_id}"),
            Some(eve),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(!sees(&w, eve).await);
    let r = t
        .req(
            "GET",
            &format!("/api/v1/items/{}/move", w.milk),
            Some(eve),
            None,
        )
        .await;
    assert_ne!(r.status, StatusCode::OK);
    // A household share can't be "left" by a member; they leave the household instead.
    let sid = s["id"].as_str().unwrap();
    let r = t
        .req("DELETE", &format!("/api/v1/shares/{sid}"), Some(bob), None)
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Deleting the household ends the share.
    t.req(
        "DELETE",
        &format!("/api/v1/households/{h}"),
        Some(&w.ann),
        None,
    )
    .await;
    assert!(!sees(&w, bob).await);

    // Only households you're in.
    let other = t
        .req(
            "POST",
            "/api/v1/households",
            Some(eve),
            Some(json!({"name": "Eve's"})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{}/shares", w.list),
            Some(&w.ann),
            Some(json!({"principal_type": "household", "principal_id": other, "role": "viewer"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn revoking_and_leaving() {
    let w = world().await;
    let t = &w.t;
    let (bob_id, bob) = &w.bob;
    let (eve_id, eve) = &w.eve;
    let sb = share(&w, "user", bob_id, "editor").await;
    let se = share(&w, "user", eve_id, "viewer").await;
    // Revoke Bob.
    let id = sb["id"].as_str().unwrap();
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/shares/{id}"),
            Some(&w.ann),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(!sees(&w, bob).await);
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lists/{}/items", w.list),
            Some(bob),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // Eve leaves by herself.
    let id = se["id"].as_str().unwrap();
    let r = t
        .req("DELETE", &format!("/api/v1/shares/{id}"), Some(eve), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(!sees(&w, eve).await);
    // Strangers learn nothing about shares.
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lists/{}/shares", w.list),
            Some(eve),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = t
        .req("DELETE", &format!("/api/v1/shares/{id}"), Some(eve), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sync_since() {
    let w = world().await;
    let t = &w.t;
    let (eve_id, eve) = &w.eve;
    share(&w, "user", eve_id, "editor").await;
    let first = t
        .req("GET", "/api/v1/sync?since=0", Some(eve), None)
        .await
        .json;
    let rev = first["rev"].as_i64().unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 1);

    // Nothing new: nothing but the list of visible lists.
    let r = t
        .req("GET", &format!("/api/v1/sync?since={rev}"), Some(eve), None)
        .await
        .json;
    assert_eq!(r["items"], json!([]));
    assert_eq!(r["lists"], json!([]));
    assert_eq!(r["visible_lists"], json!([w.list]));

    // Ann adds bread and deletes milk: Eve gets exactly that.
    t.req(
        "POST",
        &format!("/api/v1/lists/{}/items", w.list),
        Some(&w.ann),
        Some(json!({"name": "bread"})),
    )
    .await;
    t.req(
        "DELETE",
        &format!("/api/v1/items/{}", w.milk),
        Some(&w.ann),
        None,
    )
    .await;
    let r = t
        .req("GET", &format!("/api/v1/sync?since={rev}"), Some(eve), None)
        .await
        .json;
    assert_eq!(r["items"].as_array().unwrap().len(), 1);
    assert_eq!(r["items"][0]["name"], "bread");
    assert_eq!(r["deleted"], json!([{"kind": "item", "id": w.milk}]));
    assert!(r["rev"].as_i64().unwrap() > rev);

    // Ann's own things (her catalogue) don't go to Eve.
    assert_eq!(r["catalog"], json!([]));

    // Products pinned by Ann show for Eve, picture included.
    let png = {
        let img = image::RgbImage::from_pixel(64, 64, image::Rgb([1, 2, 3]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    };
    let img = t
        .raw("POST", "/api/v1/images", &w.ann, "image/png", png)
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    let pic = t
        .req("GET", &format!("/api/v1/images/{img}"), Some(eve), None)
        .await;
    assert_eq!(
        pic.status,
        StatusCode::NOT_FOUND,
        "nothing Eve sees uses it yet"
    );
    let bread = &r["items"][0];
    let entry = t
        .req("GET", "/api/v1/catalog", Some(&w.ann), None)
        .await
        .json
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "bread")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let p = t
        .req(
            "POST",
            &format!("/api/v1/catalog/{entry}/products"),
            Some(&w.ann),
            Some(json!({"brand": "Leibur", "image_id": img})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Shown by an item on a list Eve sees (as a product of bread's catalogue entry), so
    // visible to her (D-40).
    let r = t
        .req("GET", &format!("/api/v1/images/{img}"), Some(eve), None)
        .await;
    assert_eq!(r.status, StatusCode::OK);
    t.req(
        "PATCH",
        &format!("/api/v1/items/{}", bread["id"].as_str().unwrap()),
        Some(&w.ann),
        Some(json!({"product_id": p})),
    )
    .await;
    let r = t
        .req("GET", "/api/v1/sync?since=0", Some(eve), None)
        .await
        .json;
    assert_eq!(r["products"][0]["brand"], "Leibur");
    let r = t
        .req("GET", &format!("/api/v1/images/{img}"), Some(eve), None)
        .await;
    assert_eq!(r.status, StatusCode::OK);
}

/// Opens `/events` as `who` and returns the body to read events from.
async fn stream(t: &T, who: &str) -> Body {
    let mut req = Request::builder()
        .uri("/api/v1/events")
        .header(header::HOST, "localhost:3100");
    req = if who.starts_with("ticket=") {
        req.uri(format!("/api/v1/events?{who}"))
    } else {
        req.header(header::COOKIE, format!("sc_session={who}"))
    };
    let mut req = req.body(Body::empty()).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9999))));
    let res = t.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    res.into_body()
}

/// The next event's name, or None if nothing comes within a moment.
async fn next_event(body: &mut Body) -> Option<String> {
    loop {
        let frame = tokio::time::timeout(Duration::from_millis(400), body.frame())
            .await
            .ok()??
            .ok()?;
        if let Ok(data) = frame.into_data() {
            let text = String::from_utf8_lossy(&data).to_string();
            if let Some(name) = text
                .lines()
                .find_map(|l| l.strip_prefix("event: ").or(l.strip_prefix("event:")))
            {
                return Some(name.trim().to_string());
            }
        }
    }
}

#[tokio::test]
async fn live_events_only_to_those_who_see_it() {
    let w = world().await;
    let t = &w.t;
    let (eve_id, eve) = &w.eve;
    let (_, bob) = &w.bob;
    let s = share(&w, "user", eve_id, "viewer").await;

    let mut eves = stream(t, eve).await;
    let mut bobs = stream(t, bob).await;
    assert_eq!(next_event(&mut eves).await.as_deref(), Some("hello"));
    assert_eq!(next_event(&mut bobs).await.as_deref(), Some("hello"));

    t.req(
        "PATCH",
        &format!("/api/v1/items/{}", w.milk),
        Some(&w.ann),
        Some(json!({"checked": true})),
    )
    .await;
    assert_eq!(next_event(&mut eves).await.as_deref(), Some("change"));
    assert_eq!(next_event(&mut bobs).await, None, "Bob can't see the list");

    // Revoking reaches Eve once (so her app drops the list), then nothing more.
    let id = s["id"].as_str().unwrap();
    t.req(
        "DELETE",
        &format!("/api/v1/shares/{id}"),
        Some(&w.ann),
        None,
    )
    .await;
    assert_eq!(next_event(&mut eves).await.as_deref(), Some("change"));
    t.req(
        "PATCH",
        &format!("/api/v1/items/{}", w.milk),
        Some(&w.ann),
        Some(json!({"checked": false})),
    )
    .await;
    assert_eq!(next_event(&mut eves).await, None);

    // Tickets for clients that can't send headers: once, and only once.
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&w.ann),
            Some(json!({"name": "dash", "scope": "read"})),
        )
        .await
        .json["token"]
        .as_str()
        .unwrap()
        .to_string();
    let ticket = t
        .req("POST", "/api/v1/events/ticket", Some(&tok), None)
        .await;
    assert_eq!(
        ticket.status,
        StatusCode::FORBIDDEN,
        "read tokens may only GET"
    );
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&w.ann),
            Some(json!({"name": "dash2", "scope": "write"})),
        )
        .await
        .json["token"]
        .as_str()
        .unwrap()
        .to_string();
    let ticket = t
        .req("POST", "/api/v1/events/ticket", Some(&tok), None)
        .await
        .json["ticket"]
        .as_str()
        .unwrap()
        .to_string();
    let mut anns = stream(t, &format!("ticket={ticket}")).await;
    assert_eq!(next_event(&mut anns).await.as_deref(), Some("hello"));
    let mut req = Request::builder()
        .uri(format!("/api/v1/events?ticket={ticket}"))
        .header(header::HOST, "localhost:3100")
        .body(Body::empty())
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9999))));
    let res = t.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
