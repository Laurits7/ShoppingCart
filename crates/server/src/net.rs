//! Outgoing HTTP (SPEC §7.4): rustls with the ring provider and bundled web roots (no C crypto,
//! no system certificate store), timeouts, a size limit on answers, and no redirects to another
//! host. Integration targets on the LAN are allowed on purpose (a self-hosted Mealie).

use std::{sync::Arc, time::Duration};

use crate::error::{ApiResult, bad};

fn tls() -> anyhow::Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth())
}

pub const USER_AGENT: &str = concat!(
    "ShoppingCart/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Laurits7/ShoppingCart)"
);

/// For product lookups: no redirects at all.
pub fn lookup_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(tls()?)
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

/// For integrations: redirects only within the same host (e.g. http → https), at most three.
pub fn integration_client() -> anyhow::Result<reqwest::Client> {
    let same_host = reqwest::redirect::Policy::custom(|attempt| {
        let ok = attempt.previous().len() < 3
            && attempt.previous().first().map(|u| u.host_str()) == Some(attempt.url().host_str());
        if ok { attempt.follow() } else { attempt.stop() }
    });
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(tls()?)
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(same_host)
        .build()?)
}

/// Read at most `max` bytes of an answer.
pub async fn body_limited(mut res: reqwest::Response, max: usize) -> Result<Vec<u8>, String> {
    if res.content_length().is_some_and(|l| l as usize > max) {
        return Err("the answer was too large".into());
    }
    let mut out = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        if out.len() + chunk.len() > max {
            return Err("the answer was too large".into());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// A server's address as entered: http(s), no user name or password in it, no trailing slash.
pub fn base_url(s: &str) -> ApiResult<String> {
    let u = reqwest::Url::parse(s.trim())
        .map_err(|_| bad("that isn't a web address (like http://mealie:9000)"))?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err(bad("the address must start with http:// or https://"));
    }
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err(bad("give just the address, without a password, ? or #"));
    }
    Ok(u.as_str().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_urls() {
        assert_eq!(
            base_url(" http://mealie:9000/ ").unwrap(),
            "http://mealie:9000"
        );
        assert_eq!(
            base_url("https://m.example.com/mealie/").unwrap(),
            "https://m.example.com/mealie"
        );
        assert!(base_url("ftp://x").is_err());
        assert!(base_url("http://user:pw@x").is_err());
        assert!(base_url("mealie:9000").is_err());
        assert!(base_url("http://x/?a=1").is_err());
    }
}
