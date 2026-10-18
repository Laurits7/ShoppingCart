//! Pictures, products (brands) and preferred products (Phase 3b).

mod common;

use std::io::Cursor;

use axum::http::{StatusCode, header};
use common::{T, setup};
use serde_json::{Value, json};

fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([shade, (x % 256) as u8, (y % 256) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

async fn upload(t: &T, s: &str, bytes: Vec<u8>) -> Value {
    let r = t.raw("POST", "/api/v1/images", s, "image/png", bytes).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    r.json
}

/// Admin with a shop; returns (session, shop, default list).
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

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn pictures() {
    let t = setup().await;
    let s = t.admin().await;
    let img = upload(&t, &s, png(1200, 900, 10)).await;
    assert_eq!(
        (img["width"].as_i64(), img["height"].as_i64()),
        (Some(640), Some(480))
    );
    assert_eq!(img["source"], "upload");
    // The same picture again is the same picture.
    let again = upload(&t, &s, png(1200, 900, 10)).await;
    assert_eq!(again["id"], img["id"]);

    let i = id(&img);
    let r = t
        .req("GET", &format!("/api/v1/images/{i}"), Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(
        r.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    let thumb = image::load_from_memory(&r.body).unwrap();
    assert_eq!((thumb.width(), thumb.height()), (160, 120));
    let r = t
        .req(
            "GET",
            &format!("/api/v1/images/{i}?size=large"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(image::load_from_memory(&r.body).unwrap().width(), 640);

    // Only pictures, by content: an SVG (whatever it claims to be) is refused.
    let r = t
        .raw(
            "POST",
            "/api/v1/images",
            &s,
            "image/png",
            b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t
        .raw(
            "POST",
            "/api/v1/images",
            &s,
            "image/png",
            vec![0u8; 9 * 1024 * 1024],
        )
        .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);

    // Someone else's picture is invisible, and can't be attached to their things.
    let (_, eve) = t.user(&s, "eve").await;
    let r = t
        .req("GET", &format!("/api/v1/images/{i}"), Some(&eve), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let shop = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&eve),
            Some(json!({"name": "Prisma"})),
        )
        .await
        .json;
    let list = shop["shop"]["default_list_id"].as_str().unwrap();
    let milk = add(&t, &eve, list, json!({"name": "Milk"})).await;
    let entry = milk["item"]["catalog_item_id"].as_str().unwrap();
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/catalog/{entry}"),
            Some(&eve),
            Some(json!({"image_id": i})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t
        .req(
            "POST",
            &format!("/api/v1/catalog/{entry}/products"),
            Some(&eve),
            Some(json!({"brand": "Alma", "image_id": i})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn products_and_preferences() {
    let t = setup().await;
    let (s, shop, list) = with_shop(&t).await;
    let milk = add(&t, &s, &list, json!({"text": "milk 2"})).await;
    let generic = id(&milk["item"]);
    let entry = milk["item"]["catalog_item_id"]
        .as_str()
        .unwrap()
        .to_string();
    let pic = id(&upload(&t, &s, png(300, 300, 50)).await);

    let mk = |body: Value| {
        let (t, s, entry) = (&t, s.clone(), entry.clone());
        async move {
            let r = t
                .req(
                    "POST",
                    &format!("/api/v1/catalog/{entry}/products"),
                    Some(&s),
                    Some(body),
                )
                .await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.json);
            r.json
        }
    };
    let alma = mk(json!({"brand": "Alma", "name": "Piim 2.5% 1 l", "barcode": "4740 0120 12345", "image_id": pic})).await;
    assert_eq!(alma["barcode"], "4740012012345");
    let farmi = mk(json!({"brand": "Farmi"})).await;
    let (alma_id, farmi_id) = (id(&alma), id(&farmi));
    for bad in [
        json!({}),
        json!({"brand": " "}),
        json!({"brand": "x", "barcode": "12"}),
    ] {
        let r = t
            .req(
                "POST",
                &format!("/api/v1/catalog/{entry}/products"),
                Some(&s),
                Some(bad),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = t
        .req(
            "GET",
            &format!("/api/v1/catalog/{entry}/products"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/products/{farmi_id}"),
            Some(&s),
            Some(json!({"name": "Piim 3.5%", "barcode": null})),
        )
        .await;
    assert_eq!(r.json["name"], "Piim 3.5%");

    // Preferred: everywhere, and differently at this shop.
    let pref = |body: Value| {
        let (t, s, entry) = (&t, s.clone(), entry.clone());
        async move {
            t.req(
                "PUT",
                &format!("/api/v1/catalog/{entry}/preferred"),
                Some(&s),
                Some(body),
            )
            .await
        }
    };
    pref(json!({"product_id": alma_id})).await;
    let r = pref(json!({"product_id": farmi_id, "shop_id": shop})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    let data = t.req("GET", "/api/v1/shopping", Some(&s), None).await.json;
    assert_eq!(data["products"].as_array().unwrap().len(), 2);
    assert!(
        data["preferences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["shop_id"].is_null() && p["product_id"] == alma_id.as_str())
    );
    let r = pref(json!({"product_id": null, "shop_id": shop})).await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);

    // Two brands of milk can be on the list at once; the same brand adds up.
    let a = add(&t, &s, &list, json!({"product_id": alma_id})).await;
    assert_eq!(a["item"]["name"], "milk");
    assert_eq!(a["item"]["product_id"], alma_id.as_str());
    assert_ne!(id(&a["item"]), generic);
    let b = add(
        &t,
        &s,
        &list,
        json!({"name": "milk", "amount": 1, "product_id": alma_id}),
    )
    .await;
    assert_eq!(b["merged_from"], 2.0);
    // Pin the generic one to Farmi, then back to any brand.
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{generic}"),
            Some(&s),
            Some(json!({"product_id": farmi_id})),
        )
        .await;
    assert_eq!(r.json["product_id"], farmi_id.as_str());
    let r = t
        .req(
            "PATCH",
            &format!("/api/v1/items/{generic}"),
            Some(&s),
            Some(json!({"product_id": null})),
        )
        .await;
    assert!(r.json["product_id"].is_null());

    // API imports match a product only by its exact name, and use the catalogue's name.
    let tok = t
        .req(
            "POST",
            "/api/v1/tokens",
            Some(&s),
            Some(json!({"name": "mealie", "scope": "write"})),
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
            Some(json!({"text": "Farmi Piim 3.5%"})),
        )
        .await;
    assert_eq!(r.json["item"]["product_id"], farmi_id.as_str());
    assert_eq!(r.json["item"]["name"], "milk");
    let r = t
        .req(
            "POST",
            &format!("/api/v1/shops/{shop}/items"),
            Some(&tok),
            Some(json!({"text": "Farmi milk"})),
        )
        .await;
    assert!(r.json["item"]["product_id"].is_null());

    // Staples can be of a brand.
    let r = t
        .req(
            "POST",
            &format!("/api/v1/shops/{shop}/staples"),
            Some(&s),
            Some(json!({"name": "Milk", "product_id": alma_id})),
        )
        .await;
    assert_eq!(r.json["product_id"], alma_id.as_str());
    let staple = id(&r.json);

    // Deleting a product makes what used it generic.
    let r = t
        .req(
            "DELETE",
            &format!("/api/v1/products/{alma_id}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let data = t.req("GET", "/api/v1/shopping", Some(&s), None).await.json;
    assert!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["product_id"] != alma_id.as_str())
    );
    assert!(
        data["staples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == staple.as_str() && x["product_id"].is_null())
    );
    assert!(data["preferences"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn merging_and_forgetting_carry_products() {
    let t = setup().await;
    let (s, _, list) = with_shop(&t).await;
    let a = add(&t, &s, &list, json!({"name": "Tomato"})).await;
    let b = add(&t, &s, &list, json!({"name": "Tomatoes"})).await;
    let (ta, tb) = (
        a["item"]["catalog_item_id"].as_str().unwrap().to_string(),
        b["item"]["catalog_item_id"].as_str().unwrap().to_string(),
    );
    let p = t
        .req(
            "POST",
            &format!("/api/v1/catalog/{tb}/products"),
            Some(&s),
            Some(json!({"brand": "Kekkilä"})),
        )
        .await
        .json;
    t.req(
        "POST",
        "/api/v1/catalog/merge",
        Some(&s),
        Some(json!({"into": ta, "from": [tb]})),
    )
    .await;
    let r = t
        .req(
            "GET",
            &format!("/api/v1/catalog/{ta}/products"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json[0]["id"], p["id"]);

    t.req("DELETE", &format!("/api/v1/catalog/{ta}"), Some(&s), None)
        .await;
    let data = t.req("GET", "/api/v1/shopping", Some(&s), None).await.json;
    assert_eq!(data["products"], json!([]));
}

#[tokio::test]
async fn others_cannot_touch_products() {
    let t = setup().await;
    let (s, _, list) = with_shop(&t).await;
    let (_, eve) = t.user(&s, "eve").await;
    let milk = add(&t, &s, &list, json!({"name": "Milk"})).await;
    let entry = milk["item"]["catalog_item_id"]
        .as_str()
        .unwrap()
        .to_string();
    let p = id(&t
        .req(
            "POST",
            &format!("/api/v1/catalog/{entry}/products"),
            Some(&s),
            Some(json!({"brand": "Alma"})),
        )
        .await
        .json);
    for (m, path, body) in [
        ("GET", format!("/api/v1/catalog/{entry}/products"), None),
        (
            "POST",
            format!("/api/v1/catalog/{entry}/products"),
            Some(json!({"brand": "x"})),
        ),
        (
            "PUT",
            format!("/api/v1/catalog/{entry}/preferred"),
            Some(json!({"product_id": p})),
        ),
        (
            "PATCH",
            format!("/api/v1/products/{p}"),
            Some(json!({"brand": "x"})),
        ),
        ("DELETE", format!("/api/v1/products/{p}"), None),
    ] {
        let r = t.req(m, &path, Some(&eve), body).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{m} {path}");
    }
    // Someone else's product can't be pinned on your own items either.
    let shop = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&eve),
            Some(json!({"name": "Prisma"})),
        )
        .await
        .json;
    let elist = shop["shop"]["default_list_id"].as_str().unwrap();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{elist}/items"),
            Some(&eve),
            Some(json!({"product_id": p})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}
