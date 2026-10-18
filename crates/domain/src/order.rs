//! Fractional-index ordering keys (SPEC §6.5). Keys are strings over a base-62 alphabet whose
//! lexicographic order matches the intended order, so reordering an item only
//! touches that one item. Mirrored in `web/src/lib/order.ts`.

const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn idx(c: u8) -> usize {
    DIGITS
        .iter()
        .position(|&d| d == c)
        .expect("invalid order key digit")
}

/// Returns a key strictly between `a` and `b`. `a = ""` means "before everything",
/// `b = None` means "after everything". Requires `a < b` and that neither key
/// ends in `'0'` (keys produced by this function never do).
pub fn key_between(a: &str, b: Option<&str>) -> String {
    let a = a.as_bytes();
    let b = b.map(str::as_bytes);
    let mut out = Vec::new();
    midpoint(a, b, &mut out);
    String::from_utf8(out).unwrap()
}

fn midpoint(a: &[u8], b: Option<&[u8]>, out: &mut Vec<u8>) {
    if let Some(b) = b {
        // Copy the shared prefix (treating a missing digit in `a` as '0').
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(b'0') == b[n] {
            n += 1;
        }
        if n > 0 {
            out.extend_from_slice(&b[..n]);
            let rest_a = if n < a.len() { &a[n..] } else { &[][..] };
            return midpoint(rest_a, Some(&b[n..]), out);
        }
    }
    let da = a.first().map(|&c| idx(c)).unwrap_or(0);
    let db = b
        .and_then(|b| b.first())
        .map(|&c| idx(c))
        .unwrap_or(DIGITS.len());
    if db - da > 1 {
        out.push(DIGITS[(da + db) / 2]);
    } else if let Some(b) = b.filter(|b| b.len() > 1) {
        out.push(b[0]);
    } else {
        out.push(DIGITS[da]);
        midpoint(if a.is_empty() { a } else { &a[1..] }, None, out);
    }
}

/// Key placed after `last` (or the first key if there is none).
pub fn key_after(last: Option<&str>) -> String {
    key_between(last.unwrap_or(""), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        // Same values are asserted in web/src/lib/order.test.ts (TS mirror).
        assert_eq!(key_between("", None), "V");
        assert_eq!(key_between("V", None), "k");
        assert_eq!(key_between("", Some("V")), "F");
        let a = key_after(None);
        let b = key_after(Some(&a));
        assert!(a < b);
        let m = key_between(&a, Some(&b));
        assert!(a < m && m < b);
    }

    #[test]
    fn many_inserts_stay_ordered() {
        // Repeatedly insert at the front, the back and between neighbours.
        let mut keys: Vec<String> = vec![key_after(None)];
        for i in 0..500 {
            let pos = (i * 7) % (keys.len() + 1);
            let lo = if pos == 0 {
                String::new()
            } else {
                keys[pos - 1].clone()
            };
            let hi = keys.get(pos).cloned();
            let k = key_between(&lo, hi.as_deref());
            assert!(lo < k, "{lo} < {k}");
            if let Some(h) = &hi {
                assert!(&k < h, "{k} < {h}");
            }
            assert!(!k.ends_with('0'));
            keys.insert(pos, k);
        }
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn adjacent_digits() {
        let k = key_between("a", Some("b"));
        assert!("a" < k.as_str() && k.as_str() < "b");
        let k = key_between("az", Some("b"));
        assert!("az" < k.as_str() && k.as_str() < "b");
        let k = key_between("a", Some("a1"));
        assert!("a" < k.as_str() && k.as_str() < "a1");
    }
}
