//! Picture uploads (SPEC §7.4): what we accept and the sizes we store.

/// Longest side of the stored thumbnail and large picture.
pub const THUMB_PX: u32 = 160;
pub const LARGE_PX: u32 = 640;
/// Largest picture we decode (each side); bigger ones are refused before decoding.
pub const MAX_SIDE_PX: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Jpeg,
    Png,
    WebP,
}

/// The picture type from its first bytes, never from the file name or the client's
/// Content-Type. Anything else (SVG, GIF, HEIC…) is refused.
pub fn sniff(b: &[u8]) -> Option<Kind> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(Kind::WebP)
    } else {
        None
    }
}

/// The size of a `w`×`h` picture scaled to fit in `max`×`max`, never enlarged.
pub fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w <= max && h <= max {
        return (w.max(1), h.max(1));
    }
    if w >= h {
        (max, ((h as u64 * max as u64) / w as u64).max(1) as u32)
    } else {
        (((w as u64 * max as u64) / h as u64).max(1) as u32, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Kind::Jpeg));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some(Kind::Png));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Kind::WebP));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVE"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn fits() {
        assert_eq!(fit(4000, 3000, 640), (640, 480));
        assert_eq!(fit(3000, 4000, 640), (480, 640));
        assert_eq!(fit(100, 50, 640), (100, 50));
        assert_eq!(fit(10_000, 1, 160), (160, 1));
        assert_eq!(fit(640, 640, 640), (640, 640));
    }
}
