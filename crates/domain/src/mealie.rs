//! Turning a Mealie shopping-list item into a ShoppingCart item (SPEC §6.10, D-3): Mealie's
//! food, quantity and unit become name, amount and unit; without a food, the display text is
//! parsed like quick add; a label becomes the category.

use crate::amount;

/// The parts of a Mealie shopping-list item we use.
#[derive(Debug, Clone, Default)]
pub struct MealieItem {
    pub food: Option<String>,
    pub quantity: Option<f64>,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub use_abbreviation: bool,
    pub note: Option<String>,
    pub display: Option<String>,
    /// The item's label, else its food's label.
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mapped {
    pub name: String,
    pub amount: Option<f64>,
    pub unit: Option<String>,
    pub note: String,
    pub category: Option<String>,
}

fn clean(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// The item to add, or `None` if there's nothing to call it.
pub fn map(m: &MealieItem) -> Option<Mapped> {
    let quantity = m.quantity.filter(|q| q.is_finite() && *q > 0.0);
    let category = clean(&m.label);
    if let Some(food) = clean(&m.food) {
        let unit = if m.use_abbreviation {
            clean(&m.unit_abbreviation).or(clean(&m.unit_name))
        } else {
            clean(&m.unit_name).or(clean(&m.unit_abbreviation))
        };
        let unit = unit.and_then(|u| amount::normalize_unit(&u));
        return Some(Mapped {
            name: food,
            // "1 milk" is just milk; a unit always keeps its amount.
            amount: quantity.filter(|q| *q != 1.0 || unit.is_some()),
            unit,
            note: clean(&m.note).unwrap_or_default(),
            category,
        });
    }
    // No food: a free-text item ("2 cans of tomatoes", "milk 1 l").
    let text = clean(&m.note).or(clean(&m.display))?;
    let p = amount::parse(&text);
    Some(Mapped {
        name: p.name,
        amount: p.amount.or(quantity.filter(|q| *q != 1.0)),
        unit: p.unit,
        note: String::new(),
        category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foods_with_units() {
        let m = MealieItem {
            food: Some("flour".into()),
            quantity: Some(500.0),
            unit_name: Some("gram".into()),
            unit_abbreviation: Some("g".into()),
            use_abbreviation: true,
            note: Some(" for the cake ".into()),
            label: Some("Baking".into()),
            ..Default::default()
        };
        assert_eq!(
            map(&m),
            Some(Mapped {
                name: "flour".into(),
                amount: Some(500.0),
                unit: Some("g".into()),
                note: "for the cake".into(),
                category: Some("Baking".into())
            })
        );
        // Unit names are normalised too; "1 milk" is just milk.
        let m = MealieItem {
            food: Some("milk".into()),
            quantity: Some(1.0),
            ..Default::default()
        };
        assert_eq!(map(&m).unwrap().amount, None);
        let m = MealieItem {
            food: Some("milk".into()),
            quantity: Some(2.0),
            unit_name: Some("liters".into()),
            ..Default::default()
        };
        let r = map(&m).unwrap();
        assert_eq!((r.amount, r.unit.as_deref()), (Some(2.0), Some("l")));
        // Zero means "no amount given".
        let m = MealieItem {
            food: Some("salt".into()),
            quantity: Some(0.0),
            ..Default::default()
        };
        assert_eq!(map(&m).unwrap().amount, None);
    }

    #[test]
    fn free_text() {
        let m = MealieItem {
            note: Some("tomatoes 3".into()),
            quantity: Some(1.0),
            display: Some("ignored".into()),
            ..Default::default()
        };
        let r = map(&m).unwrap();
        assert_eq!((r.name.as_str(), r.amount), ("tomatoes", Some(3.0)));
        let m = MealieItem {
            display: Some("1 kg potatoes".into()),
            ..Default::default()
        };
        let r = map(&m).unwrap();
        assert_eq!(
            (r.name.as_str(), r.amount, r.unit.as_deref()),
            ("potatoes", Some(1.0), Some("kg"))
        );
        let m = MealieItem {
            note: Some("bread".into()),
            quantity: Some(2.0),
            ..Default::default()
        };
        assert_eq!(map(&m).unwrap().amount, Some(2.0));
        assert_eq!(map(&MealieItem::default()), None);
        assert_eq!(
            map(&MealieItem {
                note: Some("  ".into()),
                ..Default::default()
            }),
            None
        );
    }
}
