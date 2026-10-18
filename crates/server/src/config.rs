//! Configuration from environment variables (SPEC §3.1).

use std::{net::IpAddr, path::PathBuf};

use anyhow::{Context, bail};

pub const DEFAULT_PORT: u16 = 3100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieSecure {
    /// Secure only when the request arrived over HTTPS (via a trusted proxy).
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: IpAddr,
    pub port: u16,
    pub data_dir: PathBuf,
    pub initial_admin_user: Option<String>,
    pub initial_admin_password: Option<String>,
    /// Trust `X-Forwarded-For/-Proto/-Host` from a reverse proxy.
    pub trust_proxy: bool,
    pub cookie_secure: CookieSecure,
    /// Sliding session lifetime.
    pub session_days: i64,
    /// The address people use (e.g. `https://cart.example.org`), for links given to other apps.
    pub public_url: Option<String>,
    /// Largest picture upload accepted, in bytes (`MAX_UPLOAD_MB`).
    pub max_upload_bytes: usize,
    /// Where product pictures and brands are looked up; `None` when `PRODUCT_LOOKUP=off`.
    pub product_lookup: Option<LookupConfig>,
    /// `SECRET_KEY`, if given (otherwise `data/secret.key` is used, created on first start).
    pub secret_key: Option<String>,
}

/// The product lookup provider's addresses (Open Food Facts; tests use a fake server).
#[derive(Debug, Clone)]
pub struct LookupConfig {
    /// Full-text search (search-a-licious): `GET {search_url}?q=…`.
    pub search_url: String,
    /// One product by barcode: `GET {product_url}/{code}.json`.
    pub product_url: String,
    /// A product's public page, credited next to its pictures: `{page_url}/{code}`.
    pub page_url: String,
    /// Pictures are only ever downloaded from URLs starting with one of these.
    pub image_prefixes: Vec<String>,
}

impl LookupConfig {
    pub fn open_food_facts() -> Self {
        Self {
            search_url: "https://search.openfoodfacts.org/search".into(),
            product_url: "https://world.openfoodfacts.org/api/v3/product".into(),
            page_url: "https://world.openfoodfacts.org/product".into(),
            image_prefixes: vec!["https://images.openfoodfacts.org/".into()],
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let cookie_secure = match var("COOKIE_SECURE").as_deref().unwrap_or("auto") {
            "auto" => CookieSecure::Auto,
            "true" | "1" => CookieSecure::Always,
            "false" | "0" => CookieSecure::Never,
            other => bail!("COOKIE_SECURE must be auto, true or false (got {other:?})"),
        };
        let session_days: i64 = var("SESSION_DAYS")
            .map(|v| v.parse())
            .transpose()
            .context("SESSION_DAYS")?
            .unwrap_or(30);
        if !(1..=3650).contains(&session_days) {
            bail!("SESSION_DAYS must be between 1 and 3650");
        }
        let max_upload_mb: usize = var("MAX_UPLOAD_MB")
            .map(|v| v.parse())
            .transpose()
            .context("MAX_UPLOAD_MB")?
            .unwrap_or(8);
        if !(1..=64).contains(&max_upload_mb) {
            bail!("MAX_UPLOAD_MB must be between 1 and 64");
        }
        let product_lookup = match var("PRODUCT_LOOKUP").as_deref().unwrap_or("openfoodfacts") {
            "off" | "false" | "0" => None,
            "openfoodfacts" => Some(LookupConfig::open_food_facts()),
            other => bail!("PRODUCT_LOOKUP must be openfoodfacts or off (got {other:?})"),
        };
        Ok(Self {
            bind: var("BIND")
                .unwrap_or_else(|| "0.0.0.0".into())
                .parse()
                .context("BIND")?,
            port: var("PORT")
                .map(|p| p.parse())
                .transpose()
                .context("PORT")?
                .unwrap_or(DEFAULT_PORT),
            data_dir: var("DATA_DIR").unwrap_or_else(|| "./data".into()).into(),
            initial_admin_user: var("INITIAL_ADMIN_USER"),
            initial_admin_password: var("INITIAL_ADMIN_PASSWORD"),
            trust_proxy: matches!(var("TRUST_PROXY").as_deref(), Some("true" | "1")),
            cookie_secure,
            session_days,
            public_url: var("PUBLIC_URL").map(|u| u.trim_end_matches('/').to_string()),
            max_upload_bytes: max_upload_mb * 1024 * 1024,
            product_lookup,
            secret_key: var("SECRET_KEY"),
        })
    }

    /// Config for tests: a throwaway data directory.
    pub fn for_tests(data_dir: PathBuf) -> Self {
        Self {
            bind: "127.0.0.1".parse().unwrap(),
            port: 0,
            data_dir,
            initial_admin_user: None,
            initial_admin_password: None,
            trust_proxy: false,
            cookie_secure: CookieSecure::Auto,
            session_days: 30,
            public_url: None,
            max_upload_bytes: 8 * 1024 * 1024,
            // Tests never reach the internet; lookup tests plug in a fake provider.
            product_lookup: None,
            secret_key: None,
        }
    }
}
