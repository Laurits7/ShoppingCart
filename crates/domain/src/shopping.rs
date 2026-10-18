//! Shops, lists and categories (SPEC §6.1–6.4, D-7).

/// Name of the list every new shop starts with (its default list).
pub const DEFAULT_LIST_NAME: &str = "Shopping list";

/// Categories for a shop's very first list. Later lists copy the default list's categories.
pub const STARTER_CATEGORIES: &[&str] = &[
    "Fruit and vegetables",
    "Bread",
    "Dairy and eggs",
    "Meat and fish",
    "Dry goods",
    "Frozen",
    "Drinks",
    "Household",
];

/// Longest name of a shop, list, category, item or staple.
pub const MAX_NAME_CHARS: usize = 200;
/// Longest item note.
pub const MAX_NOTE_CHARS: usize = 1000;

/// A colour like `#1a2b3c`, or `None` to clear it.
pub fn check_color(c: &str) -> Result<Option<String>, &'static str> {
    let c = c.trim();
    if c.is_empty() {
        return Ok(None);
    }
    let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit());
    if ok {
        Ok(Some(c.to_lowercase()))
    } else {
        Err("color must look like #1a2b3c")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(check_color("#1A2b3c"), Ok(Some("#1a2b3c".into())));
        assert_eq!(check_color(""), Ok(None));
        assert!(check_color("red").is_err());
        assert!(check_color("#12345").is_err());
    }
}
