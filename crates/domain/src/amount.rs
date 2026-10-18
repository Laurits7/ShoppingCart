//! Item amounts and units (SPEC §5.2, §6.4, D-10): quick-add parsing ("milk 2", "flour 1 kg"),
//! the −/+ step rules and adding a duplicate's amount. Mirrored in `web/src/lib/amount.ts`;
//! keep the two in step (the tests use the same cases).

/// Largest amount accepted anywhere.
pub const MAX_AMOUNT: f64 = 100_000.0;
/// Longest custom unit ("bottle", "bunch", or whatever someone types).
pub const MAX_UNIT_CHARS: usize = 16;

/// Amounts are kept to three decimals, so 0.1 + 0.2 stays 0.3.
pub fn round(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// The canonical spelling of a unit. `None` means pieces (no unit): empty, "pcs", "tk"…
/// Unknown units are kept as typed (trimmed, lower-cased).
pub fn normalize_unit(u: &str) -> Option<String> {
    let u = u.trim().to_lowercase();
    if u.is_empty() {
        return None;
    }
    match known_unit(&u) {
        Some(Some(c)) => Some(c.to_string()),
        Some(None) => None,
        None => Some(u),
    }
}

/// `Some(Some(canonical))` for a known unit, `Some(None)` for a word meaning pieces, `None`
/// for anything else.
fn known_unit(u: &str) -> Option<Option<&'static str>> {
    Some(Some(match u {
        "kg" | "kgs" | "kilo" | "kilos" | "kilogram" | "kilograms" => "kg",
        "g" | "gr" | "gram" | "grams" => "g",
        "l" | "ltr" | "liter" | "liters" | "litre" | "litres" => "l",
        "dl" => "dl",
        "cl" => "cl",
        "ml" => "ml",
        "pack" | "packs" | "pk" | "pkg" => "pack",
        "bottle" | "bottles" => "bottle",
        "can" | "cans" => "can",
        "box" | "boxes" => "box",
        "bag" | "bags" => "bag",
        "jar" | "jars" => "jar",
        "bunch" | "bunches" => "bunch",
        "pc" | "pcs" | "piece" | "pieces" | "tk" => return Some(None),
        _ => return None,
    }))
}

/// A unit typed into the unit field: anything short without control characters.
pub fn check_unit(u: &str) -> Result<Option<String>, &'static str> {
    let n = normalize_unit(u);
    match &n {
        Some(s) if s.chars().count() > MAX_UNIT_CHARS || s.chars().any(char::is_control) => {
            Err("unit must be at most 16 characters")
        }
        _ => Ok(n),
    }
}

pub fn check_amount(a: f64) -> Result<f64, &'static str> {
    if a.is_finite() && a > 0.0 && a <= MAX_AMOUNT {
        Ok(round(a))
    } else {
        Err("amount must be a positive number")
    }
}

/// The −/+ step: 1 for pieces and packs, 0.5 for kg and l, 100 for g and ml.
pub fn default_step(unit: Option<&str>) -> f64 {
    match unit {
        Some("kg" | "l") => 0.5,
        Some("g" | "ml") => 100.0,
        _ => 1.0,
    }
}

/// The amount after one press of + (`up`) or −. `None` means − at the smallest step: the
/// item should go.
pub fn step(amount: f64, unit: Option<&str>, step: Option<f64>, up: bool) -> Option<f64> {
    let s = step
        .filter(|s| *s > 0.0)
        .unwrap_or_else(|| default_step(unit));
    if up {
        Some(round((amount + s).min(MAX_AMOUNT)))
    } else if amount - s < 1e-9 {
        None
    } else {
        Some(round(amount - s))
    }
}

/// Size of one unit in a base unit, for units that convert (grams, millilitres).
fn base(unit: Option<&str>) -> Option<(&'static str, f64)> {
    Some(match unit? {
        "g" => ("g", 1.0),
        "kg" => ("g", 1000.0),
        "ml" => ("ml", 1.0),
        "cl" => ("ml", 10.0),
        "dl" => ("ml", 100.0),
        "l" => ("ml", 1000.0),
        _ => return None,
    })
}

/// Adding `amount unit` to an item that has `existing unit`: the new total in the existing
/// item's unit, or `None` when the units don't mix (then it is a separate row).
pub fn combine(
    existing: f64,
    existing_unit: Option<&str>,
    amount: f64,
    unit: Option<&str>,
) -> Option<f64> {
    if existing_unit == unit {
        return Some(round((existing + amount).min(MAX_AMOUNT)));
    }
    let (eb, ef) = base(existing_unit)?;
    let (ab, af) = base(unit)?;
    (eb == ab).then(|| round((existing + amount * af / ef).min(MAX_AMOUNT)))
}

/// "2", "1.5 kg", "500 g": an amount for display (as `formatAmount` in the web app).
pub fn format(amount: f64, unit: Option<&str>) -> String {
    let a = round(amount);
    let n = if a.fract() == 0.0 {
        format!("{}", a as i64)
    } else {
        let s = format!("{a:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    match unit {
        Some(u) if !u.is_empty() => format!("{n} {u}"),
        _ => n,
    }
}

/// What a quick-add line means.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub name: String,
    pub amount: Option<f64>,
    pub unit: Option<String>,
}

fn number(s: &str) -> Option<f64> {
    let s = s.replace(',', ".");
    let ok = !s.is_empty()
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.chars().filter(|c| *c == '.').count() <= 1
        && s.chars().any(|c| c.is_ascii_digit());
    ok.then(|| s.parse().ok())
        .flatten()
        .and_then(|a| check_amount(a).ok())
}

/// "2", "2x", "x2", "×2", "1.5kg", "500g" as an amount and a unit (pieces = `None`).
fn glued(tok: &str) -> Option<(f64, Option<String>)> {
    let t = tok.to_lowercase();
    let t = t
        .strip_prefix('x')
        .or_else(|| t.strip_prefix('×'))
        .or_else(|| t.strip_suffix('x'))
        .or_else(|| t.strip_suffix('×'))
        .unwrap_or(&t);
    if let Some(a) = number(t) {
        return Some((a, None));
    }
    let split = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))?;
    let (n, u) = t.split_at(split);
    let unit = known_unit(u)?;
    Some((number(n)?, unit.map(String::from)))
}

/// Split a quick-add line into name, amount and unit. The amount (with an optional unit)
/// may come last ("milk 2", "flour 1 kg", "butter 500g") or first ("2 milk", "1 kg flour").
/// If nothing is left for the name, the whole line is the name.
pub fn parse(text: &str) -> Parsed {
    let toks: Vec<&str> = text.split_whitespace().collect();
    let whole = || Parsed {
        name: toks.join(" "),
        amount: None,
        unit: None,
    };
    let n = toks.len();
    if n < 2 {
        return whole();
    }
    // "1 kg" alone is a name, not one kilo of "kg".
    let make = |name: &[&str], (amount, unit): (f64, Option<String>)| match name {
        [only] if known_unit(&only.to_lowercase()).is_some() => whole(),
        _ => Parsed {
            name: name.join(" "),
            amount: Some(amount),
            unit,
        },
    };
    // Trailing "<number> <unit>".
    if n >= 3
        && let (Some(a), Some(u)) = (number(toks[n - 2]), known_unit(&toks[n - 1].to_lowercase()))
    {
        return make(&toks[..n - 2], (a, u.map(String::from)));
    }
    // Trailing "<number>", "<number><unit>", "x2".
    if let Some(au) = glued(toks[n - 1]) {
        return make(&toks[..n - 1], au);
    }
    // Leading "<number> <unit> name".
    if n >= 3
        && let (Some(a), Some(u)) = (number(toks[0]), known_unit(&toks[1].to_lowercase()))
    {
        return make(&toks[2..], (a, u.map(String::from)));
    }
    // Leading "<number>", "<number><unit>", "2x".
    if let Some(au) = glued(toks[0]) {
        return make(&toks[1..], au);
    }
    whole()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, amount: Option<f64>, unit: Option<&str>) -> Parsed {
        Parsed {
            name: name.into(),
            amount,
            unit: unit.map(String::from),
        }
    }

    #[test]
    fn parses_quick_add() {
        // Same cases in web/src/lib/amount.test.ts.
        assert_eq!(parse("milk"), p("milk", None, None));
        assert_eq!(parse("  milk   2 "), p("milk", Some(2.0), None));
        assert_eq!(parse("flour 1 kg"), p("flour", Some(1.0), Some("kg")));
        assert_eq!(parse("flour 1,5 KG"), p("flour", Some(1.5), Some("kg")));
        assert_eq!(parse("butter 500g"), p("butter", Some(500.0), Some("g")));
        assert_eq!(parse("2 milk"), p("milk", Some(2.0), None));
        assert_eq!(parse("1 l oat milk"), p("oat milk", Some(1.0), Some("l")));
        assert_eq!(parse("eggs x10"), p("eggs", Some(10.0), None));
        assert_eq!(parse("3x lemons"), p("lemons", Some(3.0), None));
        assert_eq!(parse("eggs 10 pcs"), p("eggs", Some(10.0), None));
        assert_eq!(parse("piim 2.5% 1 l"), p("piim 2.5%", Some(1.0), Some("l")));
        assert_eq!(
            parse("water 6 bottles"),
            p("water", Some(6.0), Some("bottle"))
        );
        // Not amounts: kept in the name.
        assert_eq!(parse("7up"), p("7up", None, None));
        assert_eq!(parse("milk 2.5%"), p("milk 2.5%", None, None));
        assert_eq!(parse("2"), p("2", None, None));
        assert_eq!(parse("1 kg"), p("1 kg", None, None));
        assert_eq!(parse("milk 0"), p("milk 0", None, None));
        assert_eq!(parse("flour 1 big"), p("flour 1 big", None, None));
        assert_eq!(parse("1.2.3 x"), p("1.2.3 x", None, None));
    }

    #[test]
    fn units() {
        assert_eq!(normalize_unit(" KG "), Some("kg".into()));
        assert_eq!(normalize_unit("pcs"), None);
        assert_eq!(normalize_unit(""), None);
        assert_eq!(normalize_unit("Tin"), Some("tin".into()));
        assert!(check_unit(&"x".repeat(17)).is_err());
        assert!(check_amount(0.0).is_err());
        assert!(check_amount(f64::NAN).is_err());
        assert_eq!(check_amount(0.1 + 0.2), Ok(0.3));
    }

    #[test]
    fn steps() {
        assert_eq!(step(1.0, None, None, true), Some(2.0));
        assert_eq!(step(1.0, None, None, false), None);
        assert_eq!(step(2.0, None, None, false), Some(1.0));
        assert_eq!(step(1.0, Some("kg"), None, false), Some(0.5));
        assert_eq!(step(0.5, Some("kg"), None, false), None);
        assert_eq!(step(0.3, Some("kg"), None, true), Some(0.8));
        assert_eq!(step(500.0, Some("g"), None, true), Some(600.0));
        assert_eq!(step(100.0, Some("g"), None, false), None);
        assert_eq!(step(2.0, Some("pack"), Some(0.25), false), Some(1.75));
        assert_eq!(step(1.0, None, Some(0.0), true), Some(2.0));
    }

    #[test]
    fn formats() {
        assert_eq!(format(2.0, None), "2");
        assert_eq!(format(1.5, Some("kg")), "1.5 kg");
        assert_eq!(format(0.1 + 0.2, Some("l")), "0.3 l");
        assert_eq!(format(500.0, Some("g")), "500 g");
    }

    #[test]
    fn combines() {
        assert_eq!(combine(2.0, None, 1.0, None), Some(3.0));
        assert_eq!(combine(1.0, Some("kg"), 500.0, Some("g")), Some(1.5));
        assert_eq!(combine(500.0, Some("g"), 1.0, Some("kg")), Some(1500.0));
        assert_eq!(combine(1.0, Some("l"), 2.0, Some("dl")), Some(1.2));
        assert_eq!(combine(1.0, Some("l"), 1.0, None), None);
        assert_eq!(combine(1.0, Some("kg"), 1.0, Some("l")), None);
        assert_eq!(combine(1.0, Some("tin"), 2.0, Some("tin")), Some(3.0));
    }
}
