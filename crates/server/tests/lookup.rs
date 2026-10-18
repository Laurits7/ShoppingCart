//! Product lookups against a fake Open Food Facts (tests never reach the internet).

mod common;

use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use common::{T, setup, setup_with};
use serde_json::{Value, json};
use shoppingcart::config::LookupConfig;

const CODE: &str = "4740125120011";

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_fn(800, 600, |x, _| image::Rgb([200, (x % 256) as u8, 30]));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

/// A fake provider on a free port; returns its config and a counter of API calls.
async fn fake() -> (LookupConfig, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let img = format!("{base}/img");
    let product = move |img: String| {
        json!({
            "code": CODE, "product_name": "Alma piim", "quantity": "1 l",
            "image_front_url": format!("{img}/large.png"), "image_front_small_url": format!("{img}/small.png"),
        })
    };
    let (c1, c2, i1, i2) = (calls.clone(), calls.clone(), img.clone(), img.clone());
    let app = Router::new()
        .route(
            "/search",
            get(
                move |Query(q): Query<std::collections::HashMap<String, String>>| {
                    c1.fetch_add(1, Ordering::SeqCst);
                    let mut p = product(i1.clone());
                    p["brands"] = json!(["Alma"]);
                    let hits = if q.get("q").is_some_and(|q| q.contains("piim")) {
                        vec![p]
                    } else {
                        vec![]
                    };
                    async move { axum::Json(json!({"hits": hits, "count": 1})) }
                },
            ),
        )
        .route(
            "/api/v3/product/{file}",
            get(move |Path(file): Path<String>| {
                c2.fetch_add(1, Ordering::SeqCst);
                let mut p = product(i2.clone());
                p["brands"] = json!("Alma, Valio");
                async move {
                    if file == format!("{CODE}.json") {
                        (
                            StatusCode::OK,
                            axum::Json(json!({"status": "success", "product": p})),
                        )
                            .into_response()
                    } else {
                        (
                            StatusCode::NOT_FOUND,
                            axum::Json(json!({"status": "failure"})),
                        )
                            .into_response()
                    }
                }
            }),
        )
        .route(
            "/img/{name}",
            get(|| async { ([(header::CONTENT_TYPE, "image/png")], png()) }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let cfg = LookupConfig {
        search_url: format!("{base}/search"),
        product_url: format!("{base}/api/v3/product"),
        page_url: "https://world.openfoodfacts.org/product".into(),
        image_prefixes: vec![format!("{img}/")],
    };
    (cfg, calls)
}

async fn shop_with_milk(t: &T) -> (String, String) {
    let s = t.admin().await;
    let r = t
        .req(
            "POST",
            "/api/v1/shops",
            Some(&s),
            Some(json!({"name": "Rimi"})),
        )
        .await;
    let list = r.json["shop"]["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = t
        .req(
            "POST",
            &format!("/api/v1/lists/{list}/items"),
            Some(&s),
            Some(json!({"name": "Milk"})),
        )
        .await;
    (
        s,
        r.json["item"]["catalog_item_id"]
            .as_str()
            .unwrap()
            .to_string(),
    )
}

#[tokio::test]
async fn search_barcode_and_import() {
    let (cfg, calls) = fake().await;
    let t = setup_with(|c| c.product_lookup = Some(cfg)).await;
    let (s, entry) = shop_with_milk(&t).await;

    let r = t
        .req("GET", "/api/v1/lookup?q=Alma%20Piim", Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json[0]["code"], CODE);
    assert_eq!(r.json[0]["brand"], "Alma");
    assert_eq!(r.json[0]["has_picture"], true);
    // The provider's picture addresses never reach the browser.
    assert!(r.json[0].get("small").is_none() && r.json[0].get("large").is_none());
    // A day's cache: asking again doesn't ask the provider again.
    t.req("GET", "/api/v1/lookup?q=alma%20%20piim", Some(&s), None)
        .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let r = t
        .req(
            "GET",
            &format!("/api/v1/lookup/picture/{CODE}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers[header::CONTENT_TYPE], "image/png");

    // Barcodes: found from the cache, unknown ones are 404 (and remembered as such).
    let r = t
        .req(
            "GET",
            &format!("/api/v1/lookup/barcode/{CODE}"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.json["name"], "Alma piim");
    let r = t
        .req("GET", "/api/v1/lookup/barcode/12345678", Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    t.req("GET", "/api/v1/lookup/barcode/12345678", Some(&s), None)
        .await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let r = t
        .req("GET", "/api/v1/lookup/barcode/12", Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Import as a product of Milk, with the picture stored on our server.
    let imp = |body: Value| {
        let (t, s) = (&t, s.clone());
        async move {
            let r = t
                .req(
                    "POST",
                    &format!("/api/v1/lookup/{CODE}/import"),
                    Some(&s),
                    Some(body),
                )
                .await;
            assert_eq!(r.status, StatusCode::OK, "{}", r.json);
            r.json
        }
    };
    let p = imp(json!({"catalog_item_id": entry})).await;
    assert_eq!(p["brand"], "Alma");
    assert_eq!(p["name"], "Alma piim 1 l");
    assert_eq!(p["barcode"], CODE);
    assert_eq!(p["source"], "openfoodfacts");
    let img = p["image_id"].as_str().unwrap().to_string();
    let r = t
        .req(
            "GET",
            &format!("/api/v1/images/{img}?size=large"),
            Some(&s),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(image::load_from_memory(&r.body).unwrap().width(), 640);
    // Importing the same barcode again doesn't make a second product.
    let again = imp(json!({"catalog_item_id": entry})).await;
    assert_eq!(again["id"], p["id"]);
    // Only the picture, for the catalogue item itself.
    let none = imp(json!({"catalog_item_id": entry, "picture_only": true})).await;
    assert!(none.is_null());
    let data = t.req("GET", "/api/v1/shopping", Some(&s), None).await.json;
    assert_eq!(data["catalog"][0]["image_id"], img.as_str());
    assert_eq!(data["products"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn off_or_unreachable() {
    // Switched off (PRODUCT_LOOKUP=off).
    let t = setup().await;
    let s = t.admin().await;
    let r = t.req("GET", "/api/v1/lookup?q=milk", Some(&s), None).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(r.json["detail"].as_str().unwrap().contains("switched off"));

    // Nobody listening.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let base = format!("http://127.0.0.1:{port}");
    let t = setup_with(|c| {
        c.product_lookup = Some(LookupConfig {
            search_url: format!("{base}/search"),
            product_url: format!("{base}/p"),
            page_url: base.clone(),
            image_prefixes: vec![],
        })
    })
    .await;
    let s = t.admin().await;
    let r = t
        .req("GET", "/api/v1/lookup?q=something", Some(&s), None)
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    // Everything else still works.
    let r = t.req("GET", "/api/v1/shopping", Some(&s), None).await;
    assert_eq!(r.status, StatusCode::OK);
}
