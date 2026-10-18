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

/// Keys past this length get the whole scope rebalanced.
pub const MAX_KEY_LEN: usize = 24;

/// `n` evenly spaced keys of a short fixed length, in order, for rebalancing a scope whose
/// keys have grown long. None ends in '0'.
pub fn spread(n: usize) -> Vec<String> {
    let base = DIGITS.len() as u128;
    let mut len = 1;
    while base.pow(len) < (n as u128 + 1) * 4 {
        len += 1;
    }
    let space = base.pow(len);
    (1..=n as u128)
        .map(|i| {
            let mut v = i * space / (n as u128 + 1);
            let mut digits = vec![b'0'; len as usize];
            for d in digits.iter_mut().rev() {
                *d = DIGITS[(v % base) as usize];
                v /= base;
            }
            // Trailing zeros carry no order information among fixed-length keys.
            while digits.last() == Some(&b'0') {
                digits.pop();
            }
            String::from_utf8(digits).unwrap()
        })
        .collect()
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

    /// A tiny deterministic generator, so the randomized tests need no extra crate.
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    #[test]
    fn random_moves_keep_order() {
        // Like users dragging things around: many random inserts and moves in several runs.
        for seed in 1..=50u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut keys: Vec<String> = spread(rng.below(20) + 1);
            for _ in 0..300 {
                if !keys.is_empty() && rng.below(2) == 0 {
                    keys.remove(rng.below(keys.len()));
                }
                let pos = rng.below(keys.len() + 1);
                let lo = if pos == 0 { "" } else { keys[pos - 1].as_str() };
                let k = key_between(lo, keys.get(pos).map(String::as_str));
                assert!(lo < k.as_str());
                assert!(keys.get(pos).is_none_or(|h| &k < h));
                assert!(!k.ends_with('0'));
                keys.insert(pos, k);
                if keys.iter().any(|k| k.len() > MAX_KEY_LEN) {
                    keys = spread(keys.len());
                }
            }
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys, sorted, "seed {seed}");
        }
    }

    #[test]
    fn worst_case_grows_slowly() {
        // Always inserting right after the same key is the worst case for key length.
        let mut keys = [key_after(None), key_after(Some("V"))];
        for _ in 0..100 {
            let k = key_between(&keys[0], Some(&keys[1]));
            keys[1] = k;
        }
        assert!(keys[1].len() <= MAX_KEY_LEN * 2, "{}", keys[1].len());
    }

    #[test]
    fn spreads() {
        for n in [0, 1, 2, 10, 61, 62, 500, 5000] {
            let keys = spread(n);
            assert_eq!(keys.len(), n);
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "n = {n}");
            assert!(keys.iter().all(|k| !k.is_empty() && !k.ends_with('0')));
            assert!(keys.iter().all(|k| k.len() <= 4));
            // There is room between neighbours and at both ends.
            if let (Some(f), Some(l)) = (keys.first(), keys.last()) {
                assert!(key_between("", Some(f)).len() <= 4);
                assert!(key_after(Some(l)).len() <= 4);
            }
        }
    }
}
