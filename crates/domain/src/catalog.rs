//! The item catalogue (SPEC §6.4a, D-10): name normalisation, suggestion ranking and the
//! frequent-items chips. Mirrored in `web/src/lib/catalog.ts`, which ranks on every keystroke
//! from a client-side copy; keep the two in step (the tests use the same cases).

/// The form names are compared in: trimmed, lower-cased, inner whitespace collapsed.
/// Diacritics are kept ("õun" and "oun" are different words in Estonian).
pub fn normalize_name(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// How a catalogue name matches what's typed; lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Match {
    Prefix,
    WordStart,
    Substring,
}

/// Both arguments normalised. An empty query matches nothing.
pub fn match_kind(name: &str, query: &str) -> Option<Match> {
    if query.is_empty() {
        None
    } else if name.starts_with(query) {
        Some(Match::Prefix)
    } else if name
        .match_indices(query)
        .any(|(i, _)| name[..i].ends_with([' ', '-', '(', '/']))
    {
        Some(Match::WordStart)
    } else if name.contains(query) {
        Some(Match::Substring)
    } else {
        None
    }
}

/// How often and how recently something was used, overall and at the current shop.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub use_count: i64,
    /// Days since it was last added anywhere.
    pub days_since_use: f64,
    /// Times added at the current shop (0 if none or no shop).
    pub shop_count: i64,
}

/// Frequency counts with diminishing returns, recency halves every 30 days, and things
/// bought at this shop get a boost.
pub fn score(u: Usage) -> f64 {
    let freq = (1.0 + u.use_count.max(0) as f64).ln();
    let recency = 0.5f64.powf(u.days_since_use.max(0.0) / 30.0);
    let shop = (1.0 + u.shop_count.max(0) as f64).ln();
    freq + 2.0 * recency + 1.5 * shop
}

/// Indices of the entries matching `query`, best first, at most `limit`. Entries are
/// `(normalised name, usage)`; the query is normalised here.
pub fn rank(entries: &[(&str, Usage)], query: &str, limit: usize) -> Vec<usize> {
    let q = normalize_name(query);
    let mut hits: Vec<(Match, f64, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, (name, u))| match_kind(name, &q).map(|m| (m, score(*u), i)))
        .collect();
    hits.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.1.total_cmp(&a.1))
            .then_with(|| entries[a.2].0.cmp(entries[b.2].0))
    });
    hits.into_iter().take(limit).map(|h| h.2).collect()
}

/// Indices of the entries bought at this shop (`shop_count > 0`), most frequent first, at
/// most `limit`: the chips shown under an empty quick-add box. The caller leaves out what is
/// already open on the list.
pub fn frequent(entries: &[(&str, Usage)], limit: usize) -> Vec<usize> {
    let mut hits: Vec<(f64, usize)> = entries
        .iter()
        .enumerate()
        .filter(|(_, (_, u))| u.shop_count > 0)
        .map(|(i, (_, u))| (score(*u), i))
        .collect();
    hits.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| entries[a.1].0.cmp(entries[b.1].0))
    });
    hits.into_iter().take(limit).map(|h| h.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(use_count: i64, days: f64, shop_count: i64) -> Usage {
        Usage {
            use_count,
            days_since_use: days,
            shop_count,
        }
    }

    #[test]
    fn normalises() {
        assert_eq!(normalize_name("  Oat   Milk "), "oat milk");
        assert_eq!(normalize_name("Õun"), "õun");
    }

    #[test]
    fn matches() {
        assert_eq!(match_kind("milk", "mi"), Some(Match::Prefix));
        assert_eq!(match_kind("oat milk", "mi"), Some(Match::WordStart));
        assert_eq!(match_kind("buttermilk", "mi"), Some(Match::Substring));
        assert_eq!(match_kind("bread", "mi"), None);
        assert_eq!(match_kind("bread", ""), None);
    }

    #[test]
    fn ranks() {
        // Same cases in web/src/lib/catalog.test.ts.
        let e = [
            ("buttermilk", u(50, 1.0, 0)),
            ("oat milk", u(3, 2.0, 0)),
            ("milk", u(1, 200.0, 0)),
            ("mint", u(10, 1.0, 0)),
            ("bread", u(99, 0.0, 0)),
        ];
        // Prefix matches first (by score), then word starts, then substrings.
        assert_eq!(rank(&e, "Mi", 10), vec![3, 2, 1, 0]);
        assert_eq!(rank(&e, "mi", 2), vec![3, 2]);
        assert_eq!(rank(&e, "", 10), Vec::<usize>::new());
        // The current shop's favourites win among equals.
        let e = [("milk", u(5, 1.0, 0)), ("mince", u(5, 1.0, 3))];
        assert_eq!(rank(&e, "mi", 10), vec![1, 0]);
    }

    #[test]
    fn scores() {
        assert!(score(u(10, 0.0, 0)) > score(u(2, 0.0, 0)));
        assert!(score(u(5, 1.0, 0)) > score(u(5, 90.0, 0)));
        assert!(score(u(5, 1.0, 2)) > score(u(5, 1.0, 0)));
    }

    #[test]
    fn frequent_items() {
        let e = [
            ("milk", u(20, 1.0, 10)),
            ("bread", u(30, 1.0, 0)),
            ("eggs", u(5, 3.0, 5)),
            ("coffee", u(5, 3.0, 5)),
        ];
        assert_eq!(frequent(&e, 10), vec![0, 3, 2]);
        assert_eq!(frequent(&e, 1), vec![0]);
    }
}
