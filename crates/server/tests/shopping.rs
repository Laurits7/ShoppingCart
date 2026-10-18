//! Shops, lists, categories, items, staples and the item catalogue (Phase 2).

mod common;

use axum::http::StatusCode;
use common::{T, setup};
use serde_json::{Value, json};

/// A signed-in admin and a shop; returns (session, shop id, default list id).
async fn with_shop(t: &T) -> (String, String, String) {
    let s = t.admin().await;
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&s),
            Some(json!({"name": "Rimi"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    let shop = r.json["shop"]["id"].as_str().unwrap().to_string();
    let list = r.json["shop"]["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    (s, shop, list)
}

async fn add(t: &T, s: &str, list: &str, body: Value) -> Value {
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            Some(s),
            Some(body),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    r.json
}

async fn items(t: &T, s: &str, list: &str) -> Vec<Value> {
    let r = t
        .req("GET", &format!("/api/v1/lists/{list}/items"), Some(s), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    r.json.as_array().unwrap().clone()
}

fn category_id(cats: &Value, name: &str) -> String {
    cats.as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no category {name} in {cats}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn ulid() -> String {
    ulid::Ulid::new().to_string()
}

#[tokio::test]
async fn shops_and_default_lists() {
    let t = setup().await;
    let s = t.admin().await;
    let (shop_id, list_id) = (ulid(), ulid());
    let body = json!({"id": shop_id, "name": " Rimi ", "color": "#AA0000", "list_id": list_id});
    let r = t
        .req("POST", "/api/v1/shops", Some(&s), Some(body.clone()))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["shop"]["name"], "Rimi");
    assert_eq!(r.json["shop"]["color"], "#aa0000");
    assert_eq!(r.json["shop"]["default_list_id"], list_id.as_str());
    assert_eq!(r.json["lists"][0]["name"], "Shopping list");
    assert_eq!(r.json["lists"][0]["is_default"], true);
    assert_eq!(r.json["lists"][0]["role"], "owner");
    // The first list gets starter categories.
    let starters = r.json["categories"].as_array().unwrap().len();
    assert!(starters >= 5);
    // Retrying the create is harmless.
    let again = t.req("POST", "/api/v1/shops", Some(&s), Some(body)).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json["lists"].as_array().unwrap().len(), 1);

    // A new list copies the default list's categories, as they are now.
    let dairy = category_id(&r.json["categories"], "Dairy and eggs");
    let ren = t
        .req(
            "PATCH",
            &format!("/api/v1/categories/{dairy}"),
            Some(&s),
            Some(json!({"name": "Dairy"})),
        )
        .await;
    assert_eq!(ren.status, StatusCode::OK);
    let r = t
        .req(
            "POST",
            "/api/v1/lists",
            Some(&s),
            Some(json!({"shop_id": shop_id, "name": "Party"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["list"]["is_default"], false);
    let party = r.json["list"]["id"].as_str().unwrap().to_string();
    let cats = r.json["categories"].as_array().unwrap();
    assert_eq!(cats.len(), starters);
    assert!(cats.iter().any(|c| c["name"] == "Dairy"));

    // The default list can't be deleted or archived.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/lists/{list_id}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/lists/{list_id}"),
            Some(&s),
            Some(json!({"archived": true})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Make the other list the default; then the first can go.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/lists/{party}"),
            Some(&s),
            Some(json!({"is_default": true})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["is_default"], true);
    let r = t
        .req("GET", &format!("/api/v1/lists/{list_id}"), Some(&s), None)
        .await;
    assert_eq!(r.json["is_default"], false);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/lists/{list_id}"),
            Some(&s),
            Some(json!({"archived": true})),
        )
        .await;
    assert!(r.json["archived_at"].is_string());
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/lists/{list_id}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t
        .req("GET", &format!("/api/v1/lists/{list_id}"), Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // Shops: rename, recolour, list, delete with everything in them.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/shops/{shop_id}"),
            Some(&s),
            Some(json!({"name": "Rimi Hyper", "color": null})),
        )
        .await;
    assert_eq!(r.json["name"], "Rimi Hyper");
    assert!(r.json["color"].is_null());
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/shops/{shop_id}"),
            Some(&s),
            Some(json!({"color": "red"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    add(&t, &s, &party, json!({"text": "milk"})).await;
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/shops/{shop_id}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t.req("GET", "/api/v1/shopping", Some(&s), None).await;
    assert_eq!(r.json["shops"], json!([]));
    assert_eq!(r.json["lists"], json!([]));
    assert_eq!(r.json["items"], json!([]));
    assert_eq!(r.json["categories"], json!([]));
    // The catalogue remembers milk even though the shop is gone.
    assert_eq!(r.json["catalog"][0]["name"], "milk");
}

#[tokio::test]
async fn quick_add_and_duplicate_guard() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;

    let a = add(&t, &s, &list, json!({"text": "Milk 2"})).await;
    assert_eq!(a["item"]["name"], "Milk");
    assert_eq!(a["item"]["amount"], 2.0);
    assert!(a["item"]["unit"].is_null());
    assert_eq!(a["item"]["source"], "manual");
    assert!(a["merged_from"].is_null());
    let milk = a["item"]["id"].as_str().unwrap().to_string();

    // The same name again adds to the amount (here the usual amount, 2).
    let id = ulid();
    let b = add(&t, &s, &list, json!({"id": id, "text": "  milk "})).await;
    assert_eq!(b["item"]["id"], milk.as_str());
    assert_eq!(b["item"]["amount"], 4.0);
    assert_eq!(b["merged_from"], 2.0);
    // Retrying that add doesn't add again.
    let c = add(&t, &s, &list, json!({"id": id, "text": "milk"})).await;
    assert_eq!(c["item"]["amount"], 4.0);

    // Units that convert are added up; others make a separate row.
    add(&t, &s, &list, json!({"text": "flour 1 kg"})).await;
    let f = add(&t, &s, &list, json!({"text": "500g flour"})).await;
    assert_eq!(f["item"]["amount"], 1.5);
    assert_eq!(f["item"]["unit"], "kg");
    let w = add(
        &t,
        &s,
        &list,
        json!({"name": "milk", "amount": 1, "unit": "l"}),
    )
    .await;
    assert!(w["merged_from"].is_null());
    assert_eq!(items(&t, &s, &list).await.len(), 3);

    // A create retried with its id is idempotent.
    let id = ulid();
    let x = add(&t, &s, &list, json!({"id": id, "name": "Bread"})).await;
    let y = add(&t, &s, &list, json!({"id": id, "name": "Bread"})).await;
    assert_eq!(x["item"], y["item"]);
    assert_eq!(items(&t, &s, &list).await.len(), 4);

    // Ticked items don't take duplicates: a new row is made.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{milk}"),
            Some(&s),
            Some(json!({"checked": true})),
        )
        .await;
    assert!(r.json["checked_at"].is_string());
    let d = add(&t, &s, &list, json!({"name": "Milk", "amount": 1})).await;
    assert_ne!(d["item"]["id"], milk.as_str());

    // Validation.
    for bad in [
        json!({}),
        json!({"name": "  "}),
        json!({"name": "x", "amount": -1}),
        json!({"name": "x", "amount": 0}),
    ] {
        let r = t
            .req(
                "POST",
                &format!("/api/v1/lists/{list}/items"),
                Some(&s),
                Some(bad),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }

    // POST /shops/{id}/items adds to the default list; with a token the source is "api".
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&s),
            Some(json!({"name": "ha", "scope": "write"})),
        )
        .await
        .json["token"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/shops/{shop}/items"),
            Some(&tok),
            Some(json!({"text": "coffee"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["item"]["list_id"], list.as_str());
    assert_eq!(r.json["item"]["source"], "api");
}

#[tokio::test]
async fn categories_are_learned() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    let cats = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            None,
        )
        .await
        .json;
    let dairy = category_id(&cats, "Dairy and eggs");
    let bread = category_id(&cats, "Bread");

    let a = add(&t, &s, &list, json!({"name": "Milk", "category_id": dairy})).await;
    assert_eq!(a["item"]["category_id"], dairy.as_str());
    // On another list of the shop (categories copied), milk finds its way to Dairy.
    let other = t
        .req(
            "POST",
            "/api/v1/lists",
            Some(&s),
            Some(json!({"shop_id": shop, "name": "Weekend"})),
        )
        .await
        .json;
    let other_list = other["list"]["id"].as_str().unwrap().to_string();
    let other_dairy = category_id(&other["categories"], "Dairy and eggs");
    let b = add(&t, &s, &other_list, json!({"text": "milk"})).await;
    assert_eq!(b["item"]["category_id"], other_dairy.as_str());
    // Unless asked for Other explicitly.
    let c = add(
        &t,
        &s,
        &list,
        json!({"name": "Butter", "category_id": null}),
    )
    .await;
    assert!(c["item"]["category_id"].is_null());

    // Moving an item teaches the catalogue.
    let butter = c["item"]["id"].as_str().unwrap();
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{butter}"),
            Some(&s),
            Some(json!({"category_id": dairy})),
        )
        .await;
    assert_eq!(r.json["category_id"], dairy.as_str());
    let d = add(&t, &s, &other_list, json!({"name": "butter"})).await;
    assert_eq!(d["item"]["category_id"], other_dairy.as_str());
    // A category of another list is refused.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{butter}"),
            Some(&s),
            Some(json!({"category_id": other_dairy})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Category names are unique per list (case-insensitively).
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            Some(json!({"name": "bread"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            Some(json!({"name": "Spices"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let all = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            None,
        )
        .await
        .json;
    assert_eq!(all.as_array().unwrap().last().unwrap()["name"], "Spices");

    // Deleting a category moves its items to Other.
    let e = add(
        &t,
        &s,
        &list,
        json!({"name": "Rye bread", "category_id": bread}),
    )
    .await;
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/categories/{bread}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let rye = items(&t, &s, &list)
        .await
        .into_iter()
        .find(|i| i["id"] == e["item"]["id"])
        .unwrap();
    assert!(rye["category_id"].is_null());
}

#[tokio::test]
async fn ticking_stepping_finishing_and_undo() {
    let t = setup().await;
    let (s, _, list) = with_shop(&t).await;
    let eggs = add(&t, &s, &list, json!({"text": "eggs 2"})).await["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let flour = add(&t, &s, &list, json!({"text": "flour 1 kg"})).await["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let step = |id: &str, by: i32| {
        let id = id.to_string();
        let t = &t;
        let s = s.clone();
        async move {
            let r = t
                .req(
                    "POST",
                    &format!("/api/v1/items/{id}/step"),
                    Some(&s),
                    Some(json!({"by": by})),
                )
                .await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.json);
            r.json
        }
    };
    assert_eq!(step(&eggs, 1).await["item"]["amount"], 3.0);
    assert_eq!(step(&flour, -1).await["item"]["amount"], 0.5);
    assert_eq!(step(&flour, 2).await["item"]["amount"], 1.5);
    // − at the smallest amount removes the item; restore brings it back.
    let r = step(&flour, -3).await;
    assert_eq!(r["removed"], true);
    assert_eq!(items(&t, &s, &list).await.len(), 1);
    let r = t
        .req(
            "POST",
            "/api/v1/items/restore",
            Some(&s),
            Some(json!({"ids": [flour]})),
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);
    assert_eq!(items(&t, &s, &list).await.len(), 2);
    let r = t
        .req(
            "POST",
            &format!("/api/v1/items/{eggs}/step"),
            Some(&s),
            Some(json!({"by": 0})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Edit fields.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{eggs}"),
            Some(&s),
            Some(json!({"amount": 10, "unit": "pcs", "note": "free range", "step": 6})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["amount"], 10.0);
    assert!(r.json["unit"].is_null());
    assert_eq!(r.json["note"], "free range");
    assert_eq!(step(&eggs, 1).await["item"]["amount"], 16.0);

    // Tick, untick, tick; open count follows.
    for checked in [true, false, true] {
        let r = t
            .req(
                "PATCH",
                &format!("/api/v1/items/{eggs}"),
                Some(&s),
                Some(json!({"checked": checked})),
            )
            .await;
        assert_eq!(r.json["checked_at"].is_string(), checked);
    }
    let r = t
        .req("GET", &format!("/api/v1/lists/{list}"), Some(&s), None)
        .await;
    assert_eq!(r.json["open_count"], 1);

    // Finish shopping removes what's in the cart; undo restores it, still ticked.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/finish"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json["removed"], json!([eggs]));
    assert_eq!(items(&t, &s, &list).await.len(), 1);
    t.req(
        "POST",
        "/api/v1/items/restore",
        Some(&s),
        Some(json!({"ids": [eggs]})),
    )
    .await;
    let all = items(&t, &s, &list).await;
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .any(|i| i["id"] == eggs.as_str() && i["checked_at"].is_string())
    );

    // Delete and restore.
    let r = t
        .req("DELETE", &format!("/api/v1/items/{flour}"), Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{flour}"),
            Some(&s),
            Some(json!({"name": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn staples() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    let mk = |body: Value| {
        let t = &t;
        let s = s.clone();
        let shop = shop.clone();
        async move {
            let r = t
                .req(
                    "POST",
                    &format!("/api/v1/shops/{shop}/staples"),
                    Some(&s),
                    Some(body),
                )
                .await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.json);
            r.json["id"].as_str().unwrap().to_string()
        }
    };
    let milk = mk(json!({"name": "Milk", "amount": 2, "category_name": "dairy and eggs"})).await;
    let coffee = mk(json!({"name": "Coffee", "unit": "pack"})).await;
    let r = t
        .req(
            "GET",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    assert_eq!(r.json[0]["name"], "Milk");

    // One milk is already open: the staple adds to it.
    add(&t, &s, &list, json!({"text": "milk"})).await;
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/staples"),
            Some(&s),
            Some(json!({"staples": [{"staple_id": milk}, {"staple_id": coffee}]})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json[0]["merged_from"], 1.0);
    assert_eq!(r.json[0]["item"]["amount"], 3.0);
    assert_eq!(r.json[1]["item"]["source"], "staple");
    assert_eq!(r.json[1]["item"]["unit"], "pack");
    // The open milk had no category, so the merged one has none either; on a fresh list
    // the staple's category is matched by name.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/staples"),
            Some(&s),
            Some(json!({"staples": [{"staple_id": milk}]})),
        )
        .await;
    assert_eq!(r.json[0]["item"]["category_id"].as_str(), None);
    let fresh = t
        .req(
            "POST",
            "/api/v1/lists",
            Some(&s),
            Some(json!({"shop_id": shop, "name": "Next"})),
        )
        .await
        .json;
    let next = fresh["list"]["id"].as_str().unwrap();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{next}/staples"),
            Some(&s),
            Some(json!({"staples": [{"staple_id": milk}]})),
        )
        .await;
    assert_eq!(
        r.json[0]["item"]["category_id"].as_str().unwrap(),
        category_id(&fresh["categories"], "Dairy and eggs")
    );

    // Edit and delete.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/staples/{coffee}"),
            Some(&s),
            Some(json!({"amount": 2, "unit": null})),
        )
        .await;
    assert_eq!(r.json["amount"], 2.0);
    assert!(r.json["unit"].is_null());
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/staples/{coffee}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t
        .req(
            "GET",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn catalog() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    for text in [
        "Milk",
        "Oat milk",
        "Buttermilk",
        "Mint",
        "Tomato",
        "Tomatoes 3",
    ] {
        add(&t, &s, &list, json!({"text": text})).await;
    }
    // Mint is bought often.
    let other = t
        .req(
            "POST",
            "/api/v1/lists",
            Some(&s),
            Some(json!({"shop_id": shop, "name": "B"})),
        )
        .await
        .json["list"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for _ in 0..3 {
        let a = add(&t, &s, &other, json!({"text": "mint"})).await;
        let id = a["item"]["id"].as_str().unwrap();
        t.req("DELETE", &format!("/api/v1/items/{id}"), Some(&s), None)
            .await;
    }

    let names = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    };
    let r = t.req("GET", "/api/v1/catalog?q=mi", Some(&s), None).await;
    assert_eq!(names(&r.json), ["Mint", "Milk", "Oat milk", "Buttermilk"]);
    assert_eq!(r.json[0]["use_count"], 4);
    assert_eq!(r.json[0]["shops"][0]["shop_id"], shop.as_str());

    // Frequent: what's bought at this shop and not open on the list.
    let r = t
        .req(
            "GET",
            &format!("/api/v1/catalog/frequent?list_id={other}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json[0]["name"], "Mint");
    let r = t
        .req(
            "GET",
            &format!("/api/v1/catalog/frequent?list_id={list}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json, json!([]));

    let all = t.req("GET", "/api/v1/catalog", Some(&s), None).await.json;
    let id_of = |n: &str| {
        all.as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == n)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (tomato, tomatoes, mint) = (id_of("Tomato"), id_of("Tomatoes"), id_of("Mint"));
    assert_eq!(
        all.as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "Tomatoes")
            .unwrap()["amount"],
        3.0
    );

    // Renaming onto another entry's name asks for a merge.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/catalog/{tomatoes}"),
            Some(&s),
            Some(json!({"name": "tomato"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = t
        .req(
            "POST",
            "/api/v1/catalog/merge",
            Some(&s),
            Some(json!({"into": tomato, "from": [tomatoes]})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["use_count"], 2);
    assert_eq!(r.json["shops"][0]["use_count"], 2);
    let linked = items(&t, &s, &list)
        .await
        .into_iter()
        .filter(|i| i["catalog_item_id"] == tomato.as_str())
        .count();
    assert_eq!(linked, 2);

    // Hidden entries aren't suggested; settings still list them.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/catalog/{mint}"),
            Some(&s),
            Some(json!({"hidden": true, "category_name": "Spices", "unit": "bunch", "amount": 1, "step": null})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["unit"], "bunch");
    let r = t.req("GET", "/api/v1/catalog?q=mi", Some(&s), None).await;
    assert!(!names(&r.json).contains(&"Mint".to_string()));
    let r = t.req("GET", "/api/v1/catalog", Some(&s), None).await;
    assert!(names(&r.json).contains(&"Mint".to_string()));

    // The usual amount and unit fill in a bare name.
    let a = add(&t, &s, &other, json!({"text": "mint"})).await;
    assert_eq!(a["item"]["unit"], "bunch");

    // Deleting forgets the entry; items stay, unlinked.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/catalog/{tomato}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = t.req("GET", "/api/v1/catalog?q=tom", Some(&s), None).await;
    assert_eq!(r.json, json!([]));
    assert!(
        items(&t, &s, &list)
            .await
            .iter()
            .filter(|i| i["name"].as_str().unwrap().starts_with("Tomato"))
            .all(|i| i["catalog_item_id"].is_null())
    );
}

/// Nobody but the owner sees or touches a shop, list, category, item, staple or catalogue
/// entry: every path answers 404 (SPEC §7.3).
#[tokio::test]
async fn others_see_nothing() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    let (_, eve) = t.user(&s, "eve").await;
    let item = add(&t, &s, &list, json!({"text": "milk"})).await["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let cats = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            None,
        )
        .await
        .json;
    let cat = cats[0]["id"].as_str().unwrap().to_string();
    let staple = t
        .req(
            "POST",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(&s),
            Some(json!({"name": "Bread"})),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_string();
    let entry = t.req("GET", "/api/v1/catalog", Some(&s), None).await.json[0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let r = t.req("GET", "/api/v1/shopping", Some(&eve), None).await;
    for k in [
        "shops",
        "lists",
        "categories",
        "items",
        "staples",
        "catalog",
    ] {
        assert_eq!(r.json[k], json!([]), "{k}");
    }
    let r = t.req("GET", "/api/v1/catalog?q=mi", Some(&eve), None).await;
    assert_eq!(r.json, json!([]));

    let calls: Vec<(&str, String, Option<Value>)> = vec![
        (
            "PATCH",
            format!("/api/v1/shops/{shop}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/shops/{shop}"), None),
        (
            "POST",
            format!("/api/v1/shops/{shop}/items"),
            Some(json!({"name": "x"})),
        ),
        ("GET", format!("/api/v1/shops/{shop}/staples"), None),
        (
            "POST",
            format!("/api/v1/shops/{shop}/staples"),
            Some(json!({"name": "x"})),
        ),
        (
            "PATCH",
            format!("/api/v1/staples/{staple}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/staples/{staple}"), None),
        (
            "POST",
            "/api/v1/lists".into(),
            Some(json!({"shop_id": shop, "name": "x"})),
        ),
        ("GET", format!("/api/v1/lists/{list}"), None),
        (
            "PATCH",
            format!("/api/v1/lists/{list}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/lists/{list}"), None),
        ("GET", format!("/api/v1/lists/{list}/categories"), None),
        (
            "POST",
            format!("/api/v1/lists/{list}/categories"),
            Some(json!({"name": "x"})),
        ),
        ("GET", format!("/api/v1/lists/{list}/items"), None),
        (
            "POST",
            format!("/api/v1/lists/{list}/items"),
            Some(json!({"name": "x"})),
        ),
        ("POST", format!("/api/v1/lists/{list}/finish"), None),
        (
            "POST",
            format!("/api/v1/lists/{list}/staples"),
            Some(json!({"staples": [{"staple_id": staple}]})),
        ),
        (
            "PATCH",
            format!("/api/v1/categories/{cat}"),
            Some(json!({"name": "x"})),
        ),
        ("DELETE", format!("/api/v1/categories/{cat}"), None),
        (
            "PATCH",
            format!("/api/v1/items/{item}"),
            Some(json!({"checked": true})),
        ),
        (
            "POST",
            format!("/api/v1/items/{item}/step"),
            Some(json!({"by": 1})),
        ),
        ("DELETE", format!("/api/v1/items/{item}"), None),
        (
            "GET",
            format!("/api/v1/catalog/frequent?list_id={list}"),
            None,
        ),
        (
            "PATCH",
            format!("/api/v1/catalog/{entry}"),
            Some(json!({"hidden": true})),
        ),
        ("DELETE", format!("/api/v1/catalog/{entry}"), None),
        (
            "POST",
            "/api/v1/catalog/merge".into(),
            Some(json!({"into": entry, "from": [entry, entry.clone() + "x"]})),
        ),
    ];
    for (method, path, body) in calls {
        let r = t.req(method, &path, Some(&eve), body).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            r.json
        );
    }
    // Restoring someone else's item is silently skipped.
    t.req("DELETE", &format!("/api/v1/items/{item}"), Some(&s), None)
        .await;
    let r = t
        .req(
            "POST",
            "/api/v1/items/restore",
            Some(&eve),
            Some(json!({"ids": [item]})),
        )
        .await;
    assert_eq!(r.json, json!([]));
    // Reusing someone else's id for a create is a conflict, not a takeover.
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&eve),
            Some(json!({"id": shop, "name": "mine"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    // The owner's things are untouched.
    let r = t.req("GET", "/api/v1/shopping", Some(&s), None).await;
    assert_eq!(r.json["shops"][0]["name"], "Rimi");
    assert_eq!(r.json["staples"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn read_tokens_only_read() {
    let t = setup().await;
    let (s, _, list) = with_shop(&t).await;
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&s),
            Some(json!({"name": "dash", "scope": "read"})),
        )
        .await
        .json["token"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/items"),
            Some(&tok),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            Some(&tok),
            Some(json!({"name": "x"})),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

/// Ids of a list's open items in display order.
async fn order(t: &T, s: &str, list: &str) -> Vec<String> {
    let mut all = items(t, s, list).await;
    all.sort_by(|a, b| {
        (a["position"].as_str(), a["id"].as_str()).cmp(&(b["position"].as_str(), b["id"].as_str()))
    });
    all.iter()
        .map(|i| i["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn moving_things() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    let mut ids = std::collections::HashMap::new();
    for n in ["a", "b", "c", "d"] {
        let r = add(&t, &s, &list, json!({"name": n, "category_id": null})).await;
        ids.insert(n, r["item"]["id"].as_str().unwrap().to_string());
    }
    let mv = |id: &str, body: Value| {
        let (t, s, id) = (&t, s.clone(), id.to_string());
        async move {
            t.req(
                "POST",
                &format!("/api/v1/items/{id}/move"),
                Some(&s),
                Some(body),
            )
            .await
        }
    };
    let r = mv(&ids["d"], json!({"after": ids["a"]})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(order(&t, &s, &list).await, ["a", "d", "b", "c"]);
    mv(&ids["c"], json!({"before": ids["a"]})).await;
    assert_eq!(order(&t, &s, &list).await, ["c", "a", "d", "b"]);
    mv(&ids["c"], json!({})).await;
    assert_eq!(order(&t, &s, &list).await, ["a", "d", "b", "c"]);
    // Moving next to itself changes nothing.
    mv(&ids["d"], json!({"after": ids["d"]})).await;
    assert_eq!(order(&t, &s, &list).await, ["a", "d", "b", "c"]);

    // Squeezing in at the same spot again and again rebalances instead of growing keys.
    for i in 0..120 {
        let who = if i % 2 == 0 { "b" } else { "c" };
        let r = mv(&ids[who], json!({"after": ids["a"]})).await;
        assert!(r.json["position"].as_str().unwrap().len() <= 24);
    }
    assert_eq!(order(&t, &s, &list).await, ["a", "c", "b", "d"]);

    // Into a category: remembered for next time.
    let cats = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            None,
        )
        .await
        .json;
    let dairy = category_id(&cats, "Dairy and eggs");
    let r = mv(&ids["a"], json!({"category_id": dairy})).await;
    assert_eq!(r.json["category_id"], dairy.as_str());
    let other = t
        .req(
            "POST",
            "/api/v1/lists",
            Some(&s),
            Some(json!({"shop_id": shop, "name": "Two"})),
        )
        .await
        .json;
    let other_list = other["list"]["id"].as_str().unwrap().to_string();
    let r = add(&t, &s, &other_list, json!({"name": "a"})).await;
    assert_eq!(
        r["item"]["category_id"],
        category_id(&other["categories"], "Dairy and eggs").as_str()
    );

    // To another list: a category of the same name is kept.
    let r = mv(&ids["a"], json!({"list_id": other_list})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["list_id"], other_list.as_str());
    assert_eq!(
        r.json["category_id"],
        category_id(&other["categories"], "Dairy and eggs").as_str()
    );
    // A sibling of the wrong list is refused.
    let r = mv(&ids["b"], json!({"after": ids["a"]})).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Several at once to a list in another shop.
    let pharmacy = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&s),
            Some(json!({"name": "Pharmacy"})),
        )
        .await
        .json;
    let p_list = pharmacy["shop"]["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "POST",
            "/api/v1/items/move",
            Some(&s),
            Some(json!({"ids": [ids["a"], ids["b"]], "list_id": p_list})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert!(
        r.json
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["list_id"] == p_list.as_str())
    );
    // Pharmacy got the starter categories too, so Dairy is kept; b had none.
    assert_eq!(
        r.json[0]["category_id"],
        category_id(&pharmacy["categories"], "Dairy and eggs").as_str()
    );
    assert!(r.json[1]["category_id"].is_null());
    assert_eq!(order(&t, &s, &p_list).await, ["a", "b"]);
    assert_eq!(order(&t, &s, &list).await, ["c", "d"]);

    // Categories, lists, shops and staples reorder the same way.
    let first = cats[0]["id"].as_str().unwrap();
    let last = cats.as_array().unwrap().last().unwrap()["id"]
        .as_str()
        .unwrap();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/categories/{first}/move"),
            Some(&s),
            Some(json!({"after": last})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let cats2 = t
        .req(
            "GET",
            &format!("/api/v1/lists/{list}/categories"),
            Some(&s),
            None,
        )
        .await
        .json;
    assert_eq!(cats2.as_array().unwrap().last().unwrap()["id"], first);

    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{other_list}/move"),
            Some(&s),
            Some(json!({"before": list})),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    let lists = t.req("GET", "/api/v1/lists", Some(&s), None).await.json;
    assert_eq!(lists[0]["id"], other_list.as_str());

    let p_shop = pharmacy["shop"]["id"].as_str().unwrap();
    t.req(
        "POST",
        &format!("/api/v1/shops/{p_shop}/move"),
        Some(&s),
        Some(json!({"before": shop})),
    )
    .await;
    let shops = t.req("GET", "/api/v1/shops", Some(&s), None).await.json;
    assert_eq!(shops[0]["id"], p_shop);

    let mut staples = vec![];
    for n in ["x", "y"] {
        staples.push(
            t.req(
                "POST",
                &format!("/api/v1/shops/{shop}/staples"),
                Some(&s),
                Some(json!({"name": n})),
            )
            .await
            .json["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    t.req(
        "POST",
        &format!("/api/v1/staples/{}/move", staples[1]),
        Some(&s),
        Some(json!({"before": staples[0]})),
    )
    .await;
    let r = t
        .req(
            "GET",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json[0]["name"], "y");
}
