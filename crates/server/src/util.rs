use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{SecondsFormat, Utc};
use rand::RngCore;
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};

use crate::error::{AppError, bad};

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}

/// Use a client-supplied ULID (lets clients create records optimistically), or make one.
pub fn id_or_new(id: Option<String>) -> Result<String, AppError> {
    match id {
        None => Ok(new_id()),
        Some(s) => ulid::Ulid::from_string(&s)
            .map(|u| u.to_string())
            .map_err(|_| bad("id must be a ULID")),
    }
}

/// 256 random bits, URL-safe.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn sha256_hex(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// A trimmed name of 1..=max characters, or a 400 naming the field.
pub fn name(field: &str, s: &str, max: usize) -> Result<String, AppError> {
    shoppingcart_domain::account::clean_name(s, max)
        .ok_or_else(|| bad(format!("{field} must be 1-{max} characters")))
}

/// For PATCH bodies: absent field = `None`, explicit `null` = `Some(None)`.
pub fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}
