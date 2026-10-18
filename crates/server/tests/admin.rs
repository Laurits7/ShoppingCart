//! Backups and the per-user export (Phase 7).

mod common;

use std::time::Duration;

use axum::http::{StatusCode, header};
use common::setup_with;
use serde_json::{Value, json};

#[tokio::test]
async fn backups_for_admins() {
    let t = setup_with(|c| c.backup_keep = 2).await;
    let admin = t.admin().await;
    let (_, eve) = t.user(&admin, "eve").await;
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&admin),
            Some(json!({"name": "x", "scope": "write"})),
        )
        .await
        .json["token"]
        .as_str()
        .unwrap()
        .to_string();

    // Only admins, only from a signed-in browser.
    for who in [&eve, &tok] {
        let r = t
            .req("POST", "/api/v1/admin/backups", Some(who), None)
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
        let r = t.req("GET", "/api/v1/admin/backups", Some(who), None).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
    }

    // Three backups a second apart; the oldest goes (keep = 2).
    let mut names = vec![];
    for _ in 0..3 {
        let r = t
            .req("POST", "/api/v1/admin/backups", Some(&admin), None)
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.json);
        names.push(r.json[0]["name"].as_str().unwrap().to_string());
        tokio::time::sleep(Duration::from_millis(1100)).await;
    }
    let list = t
        .req("GET", "/api/v1/admin/backups", Some(&admin), None)
        .await
        .json;
    let listed: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert_eq!(listed, vec![names[2].as_str(), names[1].as_str()]);

    // Download: a real SQLite file.
    let r = t
        .req(
            "GET",
            &format!("/api/v1/admin/backups/{}", names[2]),
            Some(&admin),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.starts_with(b"SQLite format 3\0"));
    assert!(
        r.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    // Nothing else in the data folder can be fetched this way.
    for bad in ["secret.key", "..%2Fsecret.key", "shoppingcart.db"] {
        let r = t
            .req(
                "GET",
                &format!("/api/v1/admin/backups/{bad}"),
                Some(&admin),
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}");
    }
    let r = t
        .req(
            "GET",
            &format!("/api/v1/admin/backups/{}", names[2]),
            Some(&eve),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn export_is_yours_only() {
    let t = setup_with(|_| {}).await;
    let ann = t.admin().await;
    let (bob_id, bob) = t.user(&ann, "bob").await;
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&ann),
            Some(json!({"name": "Rimi"})),
        )
        .await;
    let list = r.json["lists"][0]["id"].as_str().unwrap().to_string();
    t.req(
        "POST",
        &format!("/api/v1/lists/{list}/items"),
        Some(&ann),
        Some(json!({"text": "milk 2"})),
    )
    .await;
    // A gift list about Bob, shared with him by mistake through a household: still not his.
    let h = t
        .req(
            "POST",
            "/api/v1/households",
            Some(&ann),
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
        Some(&ann),
        Some(json!({"user_id": bob_id})),
    )
    .await;
    let p = t
        .req(
            "POST",
            "/api/v1/persons",
            Some(&ann),
            Some(json!({"name": "Bob", "user_id": bob_id})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    let g = t
        .req(
            "POST",
            "/api/v1/wishlists",
            Some(&ann),
            Some(json!({"kind": "gift", "title": "Ideas for Bob", "recipient_person_id": p})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    t.req(
        "POST",
        &format!("/api/v1/wishlists/{g}/items"),
        Some(&ann),
        Some(json!({"title": "A watch"})),
    )
    .await;
    t.req(
        "POST",
        &format!("/api/v1/wishlists/{g}/shares"),
        Some(&ann),
        Some(json!({"principal_type": "household", "principal_id": h, "role": "viewer"})),
    )
    .await;
    // Ann also shares her shopping list with Bob.
    t.req(
        "POST",
        &format!("/api/v1/lists/{list}/shares"),
        Some(&ann),
        Some(json!({"principal_type": "user", "principal_id": bob_id, "role": "editor"})),
    )
    .await;

    let r = t.req("GET", "/api/v1/export", Some(&ann), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        r.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("shoppingcart-admin-")
    );
    let d: &Value = &r.json["data"];
    assert_eq!(r.json["format"], "shoppingcart-export-1");
    assert_eq!(d["items"][0]["name"], "milk");
    assert_eq!(d["items"][0]["amount"], 2.0);
    assert_eq!(d["wishlists"][0]["title"], "Ideas for Bob");
    assert_eq!(d["wish_items"][0]["title"], "A watch");
    assert!(!r.body.windows(13).any(|w| w == b"password_hash"));

    // Bob's export: none of Ann's things, not even the list shared with him or the gift list.
    let r = t.req("GET", "/api/v1/export", Some(&bob), None).await;
    let text = String::from_utf8_lossy(&r.body).to_string();
    assert!(
        !text.contains("milk") && !text.contains("Ideas for Bob") && !text.contains("A watch"),
        "{text}"
    );
    assert_eq!(r.json["data"]["lists"], json!([]));
}
