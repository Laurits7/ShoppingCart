//! Rules for usernames, display names, passwords and names of things (SPEC §7.1).

pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Usernames are compared case-insensitively; this is the stored form.
pub fn normalize_username(u: &str) -> String {
    u.trim().to_lowercase()
}

pub fn check_username(u: &str) -> Result<(), &'static str> {
    let ok = !u.is_empty()
        && u.chars().count() <= 64
        && u.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err("username may contain letters, digits, '.', '_' and '-' (at most 64)")
    }
}

pub fn check_password(pw: &str) -> Result<(), &'static str> {
    if pw.chars().count() < MIN_PASSWORD_CHARS {
        Err("password must be at least 10 characters")
    } else if pw.len() > MAX_PASSWORD_BYTES {
        Err("password is too long")
    } else {
        Ok(())
    }
}

/// A trimmed name of 1..=max characters without control characters, or an error message.
pub fn clean_name(s: &str, max: usize) -> Option<String> {
    let s = s.trim();
    let n = s.chars().count();
    (n >= 1 && n <= max && !s.chars().any(char::is_control)).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        assert_eq!(normalize_username("  Laurits "), "laurits");
        assert!(check_username("laurits.t").is_ok());
        assert!(check_username("mõmmi_1").is_ok());
        assert!(check_username("").is_err());
        assert!(check_username("a b").is_err());
        assert!(check_username("a/b").is_err());
        assert!(check_username(&"x".repeat(65)).is_err());
    }

    #[test]
    fn passwords() {
        assert!(check_password("short").is_err());
        assert!(check_password("0123456789").is_ok());
        assert!(check_password(&"x".repeat(2000)).is_err());
    }

    #[test]
    fn names() {
        assert_eq!(clean_name("  Home ", 100).as_deref(), Some("Home"));
        assert_eq!(clean_name("   ", 100), None);
        assert_eq!(clean_name("a\u{0}b", 100), None);
        assert_eq!(clean_name("abc", 2), None);
    }
}
