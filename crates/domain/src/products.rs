//! Which product and picture an item shows (SPEC §6.4b). Mirrored in
//! `web/src/lib/products.ts`; keep the two in step (the tests share their cases).

/// Where an item's picture comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureFrom {
    /// The product the item is pinned to ("milk by Alma").
    Pinned,
    /// A generic item showing its preferred product, labelled as preferred.
    Preferred,
    /// The catalogue item's own picture (no brand).
    Catalog,
}

/// The product a generic item would be bought as: the one preferred at this shop, else the
/// one preferred everywhere.
pub fn preferred<'a>(at_shop: Option<&'a str>, everywhere: Option<&'a str>) -> Option<&'a str> {
    at_shop.or(everywhere)
}

/// The picture to show and where it comes from. Each argument is the picture (image id) of
/// that source, if it has one: a pinned product without a picture falls back to the
/// catalogue item's picture, never to another brand's.
pub fn picture<'a>(
    pinned: Option<Option<&'a str>>,
    preferred: Option<Option<&'a str>>,
    catalog: Option<&'a str>,
) -> Option<(&'a str, PictureFrom)> {
    match pinned {
        Some(p) => p
            .map(|i| (i, PictureFrom::Pinned))
            .or(catalog.map(|i| (i, PictureFrom::Catalog))),
        None => preferred
            .flatten()
            .map(|i| (i, PictureFrom::Preferred))
            .or(catalog.map(|i| (i, PictureFrom::Catalog))),
    }
}

/// A product's label: "Alma · Piim 2.5% 1 l", or whichever part it has.
pub fn label(brand: &str, name: &str) -> String {
    match (brand.trim(), name.trim()) {
        ("", n) => n.to_string(),
        (b, "") => b.to_string(),
        (b, n) => format!("{b} · {n}"),
    }
}

/// A barcode as typed or scanned: 8 to 14 digits (EAN-8, UPC-A, EAN-13, GTIN-14).
pub fn check_barcode(s: &str) -> Result<String, &'static str> {
    let d: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if (8..=14).contains(&d.len()) && d.chars().all(|c| c.is_ascii_digit()) {
        Ok(d)
    } else {
        Err("a barcode is 8 to 14 digits")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PictureFrom::*;

    #[test]
    fn preference() {
        assert_eq!(preferred(Some("farmi"), Some("alma")), Some("farmi"));
        assert_eq!(preferred(None, Some("alma")), Some("alma"));
        assert_eq!(preferred(None, None), None);
    }

    #[test]
    fn pictures() {
        // Same cases in web/src/lib/products.test.ts.
        assert_eq!(
            picture(Some(Some("p")), Some(Some("q")), Some("c")),
            Some(("p", Pinned))
        );
        assert_eq!(
            picture(Some(None), Some(Some("q")), Some("c")),
            Some(("c", Catalog))
        );
        assert_eq!(picture(Some(None), Some(Some("q")), None), None);
        assert_eq!(
            picture(None, Some(Some("q")), Some("c")),
            Some(("q", Preferred))
        );
        assert_eq!(picture(None, Some(None), Some("c")), Some(("c", Catalog)));
        assert_eq!(picture(None, None, None), None);
    }

    #[test]
    fn labels_and_barcodes() {
        assert_eq!(label("Alma", "Piim 2.5%"), "Alma · Piim 2.5%");
        assert_eq!(label(" ", "Piim"), "Piim");
        assert_eq!(label("Alma", ""), "Alma");
        assert_eq!(check_barcode("4740 0120-12345"), Ok("4740012012345".into()));
        assert!(check_barcode("1234567").is_err());
        assert!(check_barcode("12345678a").is_err());
    }
}
