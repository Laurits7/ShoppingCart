//! Wishlists, gift lists and claims (Phase 5), above all the privacy rules (SPEC §7.3): a gift
//! list never reaches its recipient on any path, and the owner of an own wishlist never sees
//! its claims, not even as a change.

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

/// Ann (admin), Bob, Carol and Dave. Household Family = Ann, Bob, Carol.
/// Bob's own wishlist (book) is shared with Family as viewer.
/// Ann's gift list "Ideas for Bob" (watch) is about Bob and shared with Family as editor.
struct W {
    t: T,
    ann: String,
    bob: (String, String),
    carol: (String, String),
    dave: (String, String),
    family: String,
    bobs: String,
    book: String,
    gift: String,
    watch: String,
    person: String,
}

fn id(v: &Value) -> String {
    v["id"]
        .as_str()
        .unwrap_or_else(|| panic!("no id in {v}"))
        .to_string()
}

async fn ok(t: &T, m: &str, path: &str, who: &str, body: Option<Value>) -> Value {
    let r = t.req(m, path, Some(who), body).await;
    assert!(r.status.is_success(), "{m} {path}: {} {}", r.status, r.json);
    r.json
}

async fn world() -> W {
    let t = setup().await;
    let ann = t.admin().await;
    let bob = t.user(&ann, "bob").await;
    let carol = t.user(&ann, "carol").await;
    let dave = t.user(&ann, "dave").await;
    let family = id(&ok(
        &t,
        "POST",
        "/api/v1/households",
        &ann,
        Some(json!({"name": "Family"})),
    )
    .await);
    for u in [&bob.0, &carol.0] {
        ok(
            &t,
            "POST",
            &format!("/api/v1/households/{family}/members"),
            &ann,
            Some(json!({"user_id": u})),
        )
        .await;
    }
    let bobs = id(&ok(
        &t,
        "POST",
        "/api/v1/wishlists",
        &bob.1,
        Some(json!({"kind": "own", "title": "Bob's wishes"})),
    )
    .await);
    let book = id(&ok(
        &t,
        "POST",
        &format!("/api/v1/wishlists/{bobs}/items"),
        &bob.1,
        Some(json!({"title": "A book", "url": "https://books.example/x", "price": 20, "priority": 2})),
    )
    .await);
    ok(
        &t,
        "POST",
        &format!("/api/v1/wishlists/{bobs}/shares"),
        &bob.1,
        Some(json!({"principal_type": "household", "principal_id": family, "role": "viewer"})),
    )
    .await;
    let person = id(&ok(
        &t,
        "POST",
        "/api/v1/persons",
        &ann,
        Some(json!({"name": "Bob", "user_id": bob.0})),
    )
    .await);
    let gift = id(&ok(
        &t,
        "POST",
        "/api/v1/wishlists",
        &ann,
        Some(json!({"kind": "gift", "title": "Ideas for Bob", "recipient_person_id": person})),
    )
    .await);
    let watch = id(&ok(
        &t,
        "POST",
        &format!("/api/v1/wishlists/{gift}/items"),
        &ann,
        Some(json!({"title": "A watch"})),
    )
    .await);
    ok(
        &t,
        "POST",
        &format!("/api/v1/wishlists/{gift}/shares"),
        &ann,
        Some(json!({"principal_type": "household", "principal_id": family, "role": "editor"})),
    )
    .await;
    W {
        t,
        ann,
        bob,
        carol,
        dave,
        family,
        bobs,
        book,
        gift,
        watch,
        person,
    }
}

fn ids(v: &Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_recipient_never_sees_a_gift_list() {
    let w = world().await;
    let t = &w.t;
    let bob = &w.bob.1;
    // Ann claims the watch; Carol (in Family) adds an idea.
    let claim = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wish-items/{}/claim", w.watch),
        &w.ann,
        Some(json!({})),
    )
    .await);
    ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{}/items", w.gift),
        &w.carol.1,
        Some(json!({"title": "Socks"})),
    )
    .await;

    // Everyone else in Family sees it, with the claim and the claimer's name.
    let carol = ok(t, "GET", "/api/v1/sync?since=0", &w.carol.1, None).await;
    assert!(ids(&carol, "wishlists").contains(&w.gift));
    let gift = carol["wishlists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == w.gift.as_str())
        .unwrap()
        .clone();
    assert_eq!(gift["recipient_name"], "Bob");
    assert_eq!(gift["role"], "editor");
    assert!(
        carol["claims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["user_name"] == "admin")
    );

    // Bob: not in any read.
    let all = ok(t, "GET", "/api/v1/sync?since=0", bob, None).await;
    assert_eq!(ids(&all, "wishlists"), vec![w.bobs.clone()]);
    assert!(
        !all["visible_wishlists"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == w.gift.as_str())
    );
    assert!(!ids(&all, "wish_items").contains(&w.watch));
    assert_eq!(
        all["claims"],
        json!([]),
        "no claims at all: the gift list's are hidden, his own list's too"
    );
    let r = ok(t, "GET", "/api/v1/wishlists", bob, None).await;
    assert_eq!(r.as_array().unwrap().len(), 1);
    assert!(!all.to_string().contains("Ideas for Bob") && !all.to_string().contains("A watch"));

    // Nor on any other path: everything is 404, exactly like something that doesn't exist.
    let section = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{}/sections", w.gift),
        &w.ann,
        Some(json!({"name": "Big"})),
    )
    .await);
    let paths: Vec<(&str, String, Option<Value>)> = vec![
        (
            "PATCH",
            format!("/api/v1/wishlists/{}", w.gift),
            Some(json!({"title": "x"})),
        ),
        ("DELETE", format!("/api/v1/wishlists/{}", w.gift), None),
        (
            "POST",
            format!("/api/v1/wishlists/{}/move", w.gift),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/wishlists/{}/items", w.gift),
            Some(json!({"title": "x"})),
        ),
        (
            "POST",
            format!("/api/v1/wishlists/{}/sections", w.gift),
            Some(json!({"name": "x"})),
        ),
        ("GET", format!("/api/v1/wishlists/{}/shares", w.gift), None),
        (
            "POST",
            format!("/api/v1/wishlists/{}/shares", w.gift),
            Some(json!({"principal_type": "user", "principal_id": w.dave.0, "role": "viewer"})),
        ),
        (
            "PATCH",
            format!("/api/v1/wish-sections/{section}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/wish-sections/{section}"), None),
        (
            "POST",
            format!("/api/v1/wish-sections/{section}/move"),
            Some(json!({})),
        ),
        (
            "PATCH",
            format!("/api/v1/wish-items/{}", w.watch),
            Some(json!({"title": "x"})),
        ),
        ("DELETE", format!("/api/v1/wish-items/{}", w.watch), None),
        (
            "POST",
            format!("/api/v1/wish-items/{}/move", w.watch),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/api/v1/wish-items/{}/claim", w.watch),
            Some(json!({})),
        ),
        (
            "PATCH",
            format!("/api/v1/claims/{claim}"),
            Some(json!({"status": "bought"})),
        ),
        ("DELETE", format!("/api/v1/claims/{claim}"), None),
    ];
    for (m, path, body) in paths {
        let r = t.req(m, &path, Some(bob), body).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{m} {path}: {}", r.json);
    }
    let r = ok(
        t,
        "POST",
        "/api/v1/wish-items/restore",
        bob,
        Some(json!({"ids": [w.watch]})),
    )
    .await;
    assert_eq!(r, json!([]));

    // Sharing it with him directly is refused (it would be pointless and confusing).
    let r = t
        .req(
            "POST",
            &format!("/api/v1/wishlists/{}/shares", w.gift),
            Some(&w.ann),
            Some(json!({"principal_type": "user", "principal_id": w.bob.0, "role": "viewer"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    // Nobody can make a gift list about themselves.
    let r = t
        .req(
            "POST",
            "/api/v1/persons",
            Some(bob),
            Some(json!({"name": "me", "user_id": w.bob.0})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    // His household page shows nothing about it either.
    let r = ok(t, "GET", "/api/v1/households", bob, None).await;
    assert!(!r.to_string().contains("Ideas for Bob"));
    let _ = (&w.family, &w.person);
}

#[tokio::test]
async fn owners_never_see_claims_on_their_own_wishlist() {
    let w = world().await;
    let t = &w.t;
    let bob = &w.bob.1;
    let before = ok(t, "GET", "/api/v1/sync?since=0", bob, None).await;
    let rev = before["rev"].as_i64().unwrap();
    let book_before = before["wish_items"][0].clone();

    // Carol claims the book, changes her mind, claims it again and buys it.
    let c1 = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wish-items/{}/claim", w.book),
        &w.carol.1,
        Some(json!({"note": "blue one"})),
    )
    .await);
    ok(
        t,
        "DELETE",
        &format!("/api/v1/claims/{c1}"),
        &w.carol.1,
        None,
    )
    .await;
    let c2 = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wish-items/{}/claim", w.book),
        &w.carol.1,
        Some(json!({})),
    )
    .await);
    let r = ok(
        t,
        "PATCH",
        &format!("/api/v1/claims/{c2}"),
        &w.carol.1,
        Some(json!({"status": "bought"})),
    )
    .await;
    assert_eq!(r["status"], "bought");

    // Ann (also in Family) sees who's getting it.
    let ann = ok(t, "GET", "/api/v1/sync?since=0", &w.ann, None).await;
    assert!(
        ann["claims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["user_name"] == "carol" && c["status"] == "bought")
    );
    // Ann can't claim it too.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/wish-items/{}/claim", w.book),
            Some(&w.ann),
            Some(json!({})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    // Nor release Carol's claim.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/claims/{c2}"),
            Some(&w.ann),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // Bob sees nothing changed: no claims, no released ones, and his item is untouched.
    let delta = ok(t, "GET", &format!("/api/v1/sync?since={rev}"), bob, None).await;
    assert_eq!(delta["claims"], json!([]));
    assert_eq!(delta["wish_items"], json!([]));
    assert_eq!(delta["deleted"], json!([]));
    let after = ok(t, "GET", "/api/v1/sync?since=0", bob, None).await;
    assert_eq!(
        after["wish_items"][0], book_before,
        "the item's rev and updated_at didn't move"
    );
    assert_eq!(after["claims"], json!([]));
    assert_eq!(after["wishlists"][0]["sees_claims"], false);
    // He can't claim his own wish, nor touch claims on it.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/wish-items/{}/claim", w.book),
            Some(bob),
            Some(json!({})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    for (m, body) in [
        ("PATCH", Some(json!({"status": "planned"}))),
        ("DELETE", None),
    ] {
        let r = t
            .req(m, &format!("/api/v1/claims/{c2}"), Some(bob), body)
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
    }
    // An outsider sees nothing at all.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/wish-items/{}/claim", w.book),
            Some(&w.dave.1),
            Some(json!({})),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn linking_a_person_later_hides_the_list_from_them() {
    let w = world().await;
    let t = &w.t;
    let (carol_id, carol) = &w.carol;
    let p = id(&ok(
        t,
        "POST",
        "/api/v1/persons",
        &w.dave.1,
        Some(json!({"name": "Carol"})),
    )
    .await);
    let g = id(&ok(
        t,
        "POST",
        "/api/v1/wishlists",
        &w.dave.1,
        Some(json!({"kind": "gift", "title": "For Carol", "recipient_person_id": p})),
    )
    .await);
    ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{g}/shares"),
        &w.dave.1,
        Some(json!({"principal_type": "user", "principal_id": carol_id, "role": "viewer"})),
    )
    .await;
    let r = ok(t, "GET", "/api/v1/sync?since=0", carol, None).await;
    assert!(
        ids(&r, "wishlists").contains(&g),
        "not linked yet: just a name"
    );
    // Dave notices it's her and links her account: gone for her, at once.
    ok(
        t,
        "PATCH",
        &format!("/api/v1/persons/{p}"),
        &w.dave.1,
        Some(json!({"user_id": carol_id})),
    )
    .await;
    let r = ok(t, "GET", "/api/v1/sync?since=0", carol, None).await;
    assert!(!ids(&r, "wishlists").contains(&g));
    assert!(
        !r["visible_wishlists"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == g.as_str())
    );
    let r = t
        .req(
            "GET",
            &format!("/api/v1/wishlists/{g}/shares"),
            Some(carol),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // A person with gift lists can't be removed.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/persons/{p}"),
            Some(&w.dave.1),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

async fn stream(t: &T, who: &str) -> Body {
    let mut req = Request::builder()
        .uri("/api/v1/events")
        .header(header::HOST, "localhost:3100")
        .header(header::COOKIE, format!("sc_session={who}"))
        .body(Body::empty())
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9999))));
    t.app.clone().oneshot(req).await.unwrap().into_body()
}

async fn next_event(body: &mut Body) -> Option<String> {
    loop {
        let frame = tokio::time::timeout(Duration::from_millis(400), body.frame())
            .await
            .ok()??
            .ok()?;
        if let Ok(data) = frame.into_data() {
            let text = String::from_utf8_lossy(&data).to_string();
            if let Some(name) = text.lines().find_map(|l| l.strip_prefix("event: ")) {
                return Some(name.trim().to_string());
            }
        }
    }
}

#[tokio::test]
async fn no_live_hints_either() {
    let w = world().await;
    let t = &w.t;
    let mut bob = stream(t, &w.bob.1).await;
    let mut carol = stream(t, &w.carol.1).await;
    assert_eq!(next_event(&mut bob).await.as_deref(), Some("hello"));
    assert_eq!(next_event(&mut carol).await.as_deref(), Some("hello"));

    // A change to the gift list: Carol hears, Bob doesn't.
    ok(
        t,
        "PATCH",
        &format!("/api/v1/wish-items/{}", w.watch),
        &w.ann,
        Some(json!({"price": 99})),
    )
    .await;
    assert_eq!(next_event(&mut carol).await.as_deref(), Some("change"));
    assert_eq!(next_event(&mut bob).await, None);
    // Sharing the gift list again (with Family, which Bob is in): still nothing for Bob.
    ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{}/shares", w.gift),
        &w.ann,
        Some(json!({"principal_type": "household", "principal_id": w.family, "role": "viewer"})),
    )
    .await;
    assert_eq!(next_event(&mut carol).await.as_deref(), Some("change"));
    assert_eq!(next_event(&mut bob).await, None);
    // A claim on Bob's own wishlist: Carol hears, Bob doesn't.
    ok(
        t,
        "POST",
        &format!("/api/v1/wish-items/{}/claim", w.book),
        &w.ann,
        Some(json!({})),
    )
    .await;
    assert_eq!(next_event(&mut carol).await.as_deref(), Some("change"));
    assert_eq!(next_event(&mut bob).await, None);
    // A change he made himself: he hears.
    ok(
        t,
        "PATCH",
        &format!("/api/v1/wish-items/{}", w.book),
        &w.bob.1,
        Some(json!({"note": "hardcover"})),
    )
    .await;
    assert_eq!(next_event(&mut bob).await.as_deref(), Some("change"));
}

#[tokio::test]
async fn editing_wishlists() {
    let w = world().await;
    let t = &w.t;
    let bob = &w.bob.1;
    // Sections, items, prices with the default currency, moving, received and restore.
    let books = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{}/sections", w.bobs),
        bob,
        Some(json!({"name": "Books"})),
    )
    .await);
    let r = ok(
        t,
        "PATCH",
        &format!("/api/v1/wish-items/{}", w.book),
        bob,
        Some(json!({"section_id": books, "price": 24.5})),
    )
    .await;
    assert_eq!(r["currency"], "EUR");
    assert_eq!(r["section_id"], books.as_str());
    for bad in [
        json!({"title": "x", "url": "javascript:alert(1)"}),
        json!({"title": "x", "priority": 5}),
        json!({"title": "x", "price": -1}),
    ] {
        let r = t
            .req(
                "POST",
                &format!("/api/v1/wishlists/{}/items", w.bobs),
                Some(bob),
                Some(bad),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let pen = id(&ok(
        t,
        "POST",
        &format!("/api/v1/wishlists/{}/items", w.bobs),
        bob,
        Some(json!({"title": "Pen", "currency": "usd", "price": 5})),
    )
    .await);
    let r = ok(
        t,
        "POST",
        &format!("/api/v1/wish-items/{pen}/move"),
        bob,
        Some(json!({"before": w.book})),
    )
    .await;
    assert!(r["position"].as_str().unwrap() < "V");
    let r = ok(
        t,
        "PATCH",
        &format!("/api/v1/wish-items/{pen}"),
        bob,
        Some(json!({"received": true})),
    )
    .await;
    assert!(r["received_at"].is_string());
    ok(
        t,
        "DELETE",
        &format!("/api/v1/wish-sections/{books}"),
        bob,
        None,
    )
    .await;
    let r = ok(t, "GET", "/api/v1/sync?since=0", bob, None).await;
    let book = r["wish_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == w.book.as_str())
        .unwrap()
        .clone();
    assert!(book["section_id"].is_null());
    ok(t, "DELETE", &format!("/api/v1/wish-items/{pen}"), bob, None).await;
    let r = ok(
        t,
        "POST",
        "/api/v1/wish-items/restore",
        bob,
        Some(json!({"ids": [pen]})),
    )
    .await;
    assert_eq!(r[0]["id"], pen.as_str());

    // Viewers (Family on Bob's list) read and claim but don't edit; only Bob manages.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/wishlists/{}/items", w.bobs),
            Some(&w.carol.1),
            Some(json!({"title": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/wishlists/{}", w.gift),
            Some(&w.carol.1),
            Some(json!({"title": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "editors don't manage");
    // Deleting a wishlist takes its items.
    ok(
        t,
        "DELETE",
        &format!("/api/v1/wishlists/{}", w.bobs),
        bob,
        None,
    )
    .await;
    let r = ok(t, "GET", "/api/v1/sync?since=0", &w.carol.1, None).await;
    assert!(!ids(&r, "wishlists").contains(&w.bobs));
    assert!(!ids(&r, "wish_items").contains(&w.book));
}
