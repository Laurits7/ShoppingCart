//! Wish items (SPEC §5.3, §6.9): links, prices and priorities.

/// Longest link.
pub const MAX_URL: usize = 2000;
pub const MAX_PRICE: f64 = 1_000_000_000.0;

/// A link to show and follow: only http(s), with something after the scheme.
pub fn check_url(s: &str) -> Result<Option<String>, &'static str> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let lower = s.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or("a link must start with http:// or https://")?;
    if rest.is_empty()
        || s.len() > MAX_URL
        || s.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err("that doesn't look like a link");
    }
    Ok(Some(s.to_string()))
}

/// The host of a link, shown instead of fetching a preview ("www.example.com" → "example.com").
pub fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (!host.is_empty()).then_some(host)
}

/// A currency code like EUR (three letters, upper-cased).
pub fn check_currency(s: &str) -> Result<String, &'static str> {
    let c = s.trim().to_ascii_uppercase();
    if c.len() == 3 && c.chars().all(|c| c.is_ascii_uppercase()) {
        Ok(c)
    } else {
        Err("currency must be a three-letter code like EUR")
    }
}

pub fn check_price(p: f64) -> Result<f64, &'static str> {
    if p.is_finite() && (0.0..=MAX_PRICE).contains(&p) {
        Ok((p * 100.0).round() / 100.0)
    } else {
        Err("price must be 0 or more")
    }
}

/// 0 = normal, 1 = would like a lot, 2 = most wanted.
pub fn check_priority(p: i64) -> Result<i64, &'static str> {
    if (0..=2).contains(&p) {
        Ok(p)
    } else {
        Err("priority must be 0, 1 or 2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            check_url(" https://example.com/x "),
            Ok(Some("https://example.com/x".into()))
        );
        assert_eq!(check_url(""), Ok(None));
        assert!(check_url("javascript:alert(1)").is_err());
        assert!(check_url("ftp://x").is_err());
        assert!(check_url("https://").is_err());
        assert!(check_url("https://a b").is_err());
        assert_eq!(
            host("https://www.Example.com:8080/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(host("https://user@shop.ee/x").as_deref(), Some("shop.ee"));
    }

    #[test]
    fn money_and_priority() {
        assert_eq!(check_currency(" eur"), Ok("EUR".into()));
        assert!(check_currency("EURO").is_err());
        assert_eq!(check_price(19.999), Ok(20.0));
        assert!(check_price(-1.0).is_err());
        assert!(check_priority(3).is_err());
    }
}
