//! Product lookups on Open Food Facts (SPEC §6.4b, §7.4): search by name and by barcode,
//! server-side only. Only the search text or barcode is sent, never anything personal.
//! Requests have timeouts and size limits and no redirects; pictures come only from the
//! provider's picture host. Results are cached for a day, and we stay under the provider's
//! rate limits (10 searches and 15 product reads a minute).

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use shoppingcart_domain::{catalog::normalize_name, products::check_barcode};
use ts_rs::TS;

use crate::{
    AppState,
    config::LookupConfig,
    error::{ApiResult, AppError, bad},
    util::now,
};

/// Largest API answer and picture we read.
const MAX_JSON: usize = 2 * 1024 * 1024;
const MAX_PICTURE: usize = 4 * 1024 * 1024;
const CACHE_HOURS: i64 = 24;
const USER_AGENT: &str = concat!(
    "ShoppingCart/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Laurits7/ShoppingCart)"
);

/// The HTTP client for lookups: rustls with the ring provider and bundled web roots.
pub fn client() -> anyhow::Result<reqwest::Client> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

/// One product found, as shown in the picker.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct LookupResult {
    /// The barcode.
    pub code: String,
    pub brand: String,
    pub name: String,
    /// "1 l", "500 g".
    pub quantity: String,
    /// A small picture is available at `/lookup/picture/{code}`.
    pub has_picture: bool,
    /// The product's page on Open Food Facts (credit for its pictures, CC BY-SA).
    pub page_url: String,
    /// Where the pictures are (kept on the server; browsers never load them from there).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(skip)]
    #[schema(ignore)]
    pub small: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(skip)]
    #[schema(ignore)]
    pub large: Option<String>,
}

impl LookupResult {
    /// The same without the provider's picture addresses, for clients.
    fn public(mut self) -> Self {
        self.small = None;
        self.large = None;
        self
    }
}

/// The provider's product fields we use. `brands` is a list in search answers and a
/// comma-separated text in product answers.
#[derive(Deserialize)]
struct RawProduct {
    code: Option<String>,
    product_name: Option<String>,
    #[serde(default)]
    brands: Option<serde_json::Value>,
    quantity: Option<String>,
    image_front_url: Option<String>,
    image_front_small_url: Option<String>,
}

fn convert(cfg: &LookupConfig, r: RawProduct) -> Option<LookupResult> {
    let code = r.code.filter(|c| check_barcode(c).is_ok())?;
    let brand = match r.brands {
        Some(serde_json::Value::Array(a)) => {
            a.first().and_then(|b| b.as_str()).unwrap_or("").to_string()
        }
        Some(serde_json::Value::String(s)) => s.split(',').next().unwrap_or("").to_string(),
        _ => String::new(),
    };
    let name = r.product_name.unwrap_or_default();
    if brand.trim().is_empty() && name.trim().is_empty() {
        return None;
    }
    let allowed = |u: &String| cfg.image_prefixes.iter().any(|p| u.starts_with(p.as_str()));
    let small = r.image_front_small_url.filter(allowed);
    let large = r.image_front_url.filter(allowed);
    Some(LookupResult {
        page_url: format!("{}/{code}", cfg.page_url),
        code,
        brand: brand.trim().chars().take(200).collect(),
        name: name.trim().chars().take(200).collect(),
        quantity: r
            .quantity
            .unwrap_or_default()
            .trim()
            .chars()
            .take(50)
            .collect(),
        has_picture: small.is_some() || large.is_some(),
        small,
        large,
    })
}

const FIELDS: &str = "code,product_name,brands,quantity,image_front_url,image_front_small_url";

/// Requests in the last minute, per kind, to stay under the provider's limits.
static RECENT_SEARCHES: Mutex<VecDeque<Instant>> = Mutex::new(VecDeque::new());
static RECENT_PRODUCTS: Mutex<VecDeque<Instant>> = Mutex::new(VecDeque::new());

fn throttle(recent: &Mutex<VecDeque<Instant>>, per_minute: usize) -> ApiResult<()> {
    let mut q = recent.lock().unwrap_or_else(|e| e.into_inner());
    while q
        .front()
        .is_some_and(|t| t.elapsed() > Duration::from_secs(60))
    {
        q.pop_front();
    }
    if q.len() >= per_minute {
        return Err(AppError::Unavailable(
            "Open Food Facts allows only a few lookups a minute; try again shortly".into(),
        ));
    }
    q.push_back(Instant::now());
    Ok(())
}

fn provider(state: &AppState) -> ApiResult<(&LookupConfig, &reqwest::Client)> {
    match (&state.config.product_lookup, &state.http) {
        (Some(c), Some(h)) => Ok((c, h)),
        _ => Err(AppError::Unavailable(
            "product lookups are switched off on this server (PRODUCT_LOOKUP=off)".into(),
        )),
    }
}

/// GET a URL and read at most `max` bytes of a successful answer.
async fn fetch(http: &reqwest::Client, url: &str, max: usize) -> ApiResult<Option<Vec<u8>>> {
    let unreachable = |e: reqwest::Error| {
        tracing::warn!("lookup failed: {e}");
        AppError::Unavailable("Open Food Facts could not be reached".into())
    };
    let mut res = http.get(url).send().await.map_err(unreachable)?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !res.status().is_success() {
        tracing::warn!("lookup answered {} for {url}", res.status());
        return Err(AppError::Unavailable(format!(
            "Open Food Facts answered {}",
            res.status().as_u16()
        )));
    }
    if res.content_length().is_some_and(|l| l as usize > max) {
        return Err(AppError::Unavailable("the answer was too large".into()));
    }
    let mut out = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(unreachable)? {
        if out.len() + chunk.len() > max {
            return Err(AppError::Unavailable("the answer was too large".into()));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(Some(out))
}

async fn cached(state: &AppState, key: &str) -> sqlx::Result<Option<Vec<LookupResult>>> {
    let since = (chrono::Utc::now() - chrono::Duration::hours(CACHE_HOURS))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let body: Option<String> =
        sqlx::query_scalar("SELECT body FROM lookup_cache WHERE key = ? AND fetched_at > ?")
            .bind(key)
            .bind(since)
            .fetch_optional(&state.db.read)
            .await?;
    Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
}

async fn remember(state: &AppState, key: &str, results: &[LookupResult]) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO lookup_cache (key, body, fetched_at) VALUES (?,?,?)
         ON CONFLICT (key) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(serde_json::to_string(results).unwrap_or_default())
    .bind(now())
    .execute(&state.db.write)
    .await?;
    // Each product is also remembered by its barcode, for its picture and for importing.
    for r in results {
        sqlx::query(
            "INSERT INTO lookup_cache (key, body, fetched_at) VALUES (?,?,?)
             ON CONFLICT (key) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
        )
        .bind(format!("code:{}", r.code))
        .bind(serde_json::to_string(&[r]).unwrap_or_default())
        .bind(now())
        .execute(&state.db.write)
        .await?;
    }
    Ok(())
}

/// Search products by name (at most 12).
pub async fn search(state: &AppState, q: &str) -> ApiResult<Vec<LookupResult>> {
    let q = normalize_name(q);
    if q.is_empty() || q.chars().count() > 100 {
        return Err(bad("search for 1 to 100 characters"));
    }
    let key = format!("q:{q}");
    if let Some(hit) = cached(state, &key).await? {
        return Ok(hit.into_iter().map(LookupResult::public).collect());
    }
    let (cfg, http) = provider(state)?;
    throttle(&RECENT_SEARCHES, 8)?;
    let mut url = reqwest::Url::parse(&cfg.search_url).map_err(anyhow::Error::from)?;
    url.query_pairs_mut()
        .append_pair("q", &q)
        .append_pair("page_size", "12")
        .append_pair("langs", "et,en")
        .append_pair("fields", FIELDS);
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default)]
        hits: Vec<RawProduct>,
    }
    let body = fetch(http, url.as_str(), MAX_JSON)
        .await?
        .unwrap_or_default();
    let answer: Answer = serde_json::from_slice(&body).map_err(|_| {
        AppError::Unavailable("Open Food Facts gave an answer we can't read".into())
    })?;
    let results: Vec<LookupResult> = answer
        .hits
        .into_iter()
        .filter_map(|r| convert(cfg, r))
        .collect();
    remember(state, &key, &results).await?;
    Ok(results.into_iter().map(LookupResult::public).collect())
}

/// The product with this barcode, with the provider's picture addresses (server use).
pub async fn by_code(state: &AppState, code: &str) -> ApiResult<Option<LookupResult>> {
    let code = check_barcode(code).map_err(bad)?;
    let key = format!("code:{code}");
    if let Some(hit) = cached(state, &key).await? {
        return Ok(hit.into_iter().next());
    }
    let (cfg, http) = provider(state)?;
    throttle(&RECENT_PRODUCTS, 12)?;
    let url = format!("{}/{code}.json?fields={FIELDS}", cfg.product_url);
    #[derive(Deserialize)]
    struct Answer {
        product: Option<RawProduct>,
    }
    let Some(body) = fetch(http, &url, MAX_JSON).await? else {
        remember(state, &key, &[]).await?;
        return Ok(None);
    };
    let answer: Answer = serde_json::from_slice(&body).map_err(|_| {
        AppError::Unavailable("Open Food Facts gave an answer we can't read".into())
    })?;
    let found = answer.product.and_then(|mut p| {
        p.code.get_or_insert_with(|| code.clone());
        convert(cfg, p)
    });
    // Remember misses too, so scanning an unknown barcode twice asks only once.
    let list: Vec<LookupResult> = found.iter().cloned().collect();
    remember(state, &key, &list).await?;
    Ok(found)
}

pub fn public(r: LookupResult) -> LookupResult {
    r.public()
}

/// Download a product's picture from the provider (small for the picker, large to keep).
pub async fn picture(state: &AppState, r: &LookupResult, large: bool) -> ApiResult<Vec<u8>> {
    let (cfg, http) = provider(state)?;
    let url = if large {
        r.large.as_ref().or(r.small.as_ref())
    } else {
        r.small.as_ref().or(r.large.as_ref())
    };
    let url = url
        .filter(|u| cfg.image_prefixes.iter().any(|p| u.starts_with(p.as_str())))
        .ok_or(AppError::NotFound)?;
    fetch(http, url, MAX_PICTURE)
        .await?
        .ok_or(AppError::NotFound)
}

/// Forget cached lookups older than a day.
pub async fn purge(state: &AppState) -> sqlx::Result<()> {
    let since = (chrono::Utc::now() - chrono::Duration::hours(CACHE_HOURS))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    sqlx::query("DELETE FROM lookup_cache WHERE fetched_at < ?")
        .bind(since)
        .execute(&state.db.write)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_provider_answers() {
        let cfg = LookupConfig::open_food_facts();
        let search: RawProduct = serde_json::from_value(serde_json::json!({
            "code": "4740125120011", "brands": ["Alma", "Valio"], "product_name": "Alma piim",
            "image_front_small_url": "https://images.openfoodfacts.org/images/x.200.jpg"
        }))
        .unwrap();
        let r = convert(&cfg, search).unwrap();
        assert_eq!((r.brand.as_str(), r.name.as_str()), ("Alma", "Alma piim"));
        assert!(r.has_picture);
        assert_eq!(
            r.page_url,
            "https://world.openfoodfacts.org/product/4740125120011"
        );
        // A product answer: brands as text; a picture elsewhere is ignored.
        let product: RawProduct = serde_json::from_value(serde_json::json!({
            "code": "4740125120011", "brands": "Farmi, Other", "product_name": "Piim",
            "image_front_url": "https://evil.example/x.jpg"
        }))
        .unwrap();
        let r = convert(&cfg, product).unwrap();
        assert_eq!(r.brand, "Farmi");
        assert!(!r.has_picture);
        // Without a usable barcode or any name, it's skipped.
        for bad in [
            serde_json::json!({"code": "12", "product_name": "x"}),
            serde_json::json!({"code": "4740125120011"}),
        ] {
            assert!(convert(&cfg, serde_json::from_value(bad).unwrap()).is_none());
        }
    }
}
