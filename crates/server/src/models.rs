//! Database rows and API DTOs. Types marked `#[ts(export)]` are exported to
//! `web/src/lib/api/types` by `cargo test` (see `.cargo/config.toml`).

use serde::Serialize;
use sqlx::FromRow;
use ts_rs::TS;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub locale: String,
    pub prefs: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub rev: i64,
}

/// The signed-in user as seen by themselves (and by admins in the user list).
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Me {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub is_admin: bool,
    /// BCP 47 language tag for dates and numbers; empty = the browser's language.
    pub locale: String,
    /// Free-form UI preferences (theme, last opened list…), shared across devices.
    #[ts(type = "Record<string, unknown>")]
    #[schema(value_type = Object)]
    pub prefs: serde_json::Value,
    pub created_at: String,
}

impl From<&User> for Me {
    fn from(u: &User) -> Self {
        Me {
            id: u.id.clone(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            is_admin: u.is_admin,
            locale: u.locale.clone(),
            prefs: serde_json::from_str(&u.prefs).unwrap_or_else(|_| serde_json::json!({})),
            created_at: u.created_at.clone(),
        }
    }
}

/// Another user as anyone signed in may see them (to add to a household or share with).
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    /// `read` (GET only) or `write`.
    #[ts(type = "'read' | 'write'")]
    pub scope: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// A signed-in browser or device.
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
    pub user_agent: Option<String>,
    /// The session this request was made with.
    #[sqlx(skip)]
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct HouseholdMember {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    #[ts(type = "'owner' | 'member'")]
    pub role: String,
}

/// A group of people to share lists and wishlists with (SPEC §5.1).
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Household {
    pub id: String,
    pub name: String,
    pub members: Vec<HouseholdMember>,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

// ---- shopping (SPEC §5.2) ---------------------------------------------------------

/// A place to buy things. Shops are personal: only their owner sees them.
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Shop {
    pub id: String,
    pub name: String,
    /// `#rrggbb`, or none.
    pub color: Option<String>,
    /// Fractional-index ordering key.
    pub position: String,
    /// Where items go when no list is chosen (SPEC §6.2).
    pub default_list_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

/// A shopping list in a shop. Lists are the unit of sharing.
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ShoppingList {
    pub id: String,
    pub shop_id: String,
    pub owner_id: String,
    pub name: String,
    pub position: String,
    pub archived_at: Option<String>,
    /// The shop's default list.
    pub is_default: bool,
    /// Your relation to the list: what you may do with it.
    #[ts(type = "'owner' | 'editor' | 'viewer'")]
    pub role: String,
    /// Items not yet in the cart.
    #[ts(type = "number")]
    pub open_count: i64,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

/// A section of a list ("Dairy"). Items without one show under *Other*.
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Category {
    pub id: String,
    pub list_id: String,
    pub name: String,
    pub position: String,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Item {
    pub id: String,
    pub list_id: String,
    pub category_id: Option<String>,
    pub catalog_item_id: Option<String>,
    pub name: String,
    pub amount: f64,
    /// "kg", "g", "l", "pack"…; none means pieces.
    pub unit: Option<String>,
    /// Overrides the −/+ step that comes from the unit.
    pub step: Option<f64>,
    pub note: String,
    pub position: String,
    /// Set while the item is in the cart.
    pub checked_at: Option<String>,
    pub checked_by: Option<String>,
    pub added_by: String,
    #[ts(type = "'manual' | 'staple' | 'mealie' | 'api'")]
    pub source: String,
    pub external_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

/// One of a shop's usual purchases (SPEC §6.3).
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Staple {
    pub id: String,
    pub shop_id: String,
    pub catalog_item_id: Option<String>,
    pub name: String,
    pub amount: f64,
    pub unit: Option<String>,
    /// Added into the list's category of this name, if it has one.
    pub category_name: Option<String>,
    pub position: String,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}

/// Times a catalogue item was added at one shop.
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct CatalogShopUse {
    pub shop_id: String,
    #[ts(type = "number")]
    pub use_count: i64,
    pub last_used_at: String,
}

/// Something you added before, remembered for suggestions (SPEC §6.4a).
#[derive(Debug, Clone, Serialize, FromRow, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    /// The category it was last put in.
    pub category_name: Option<String>,
    /// Usual amount and unit.
    pub amount: f64,
    pub unit: Option<String>,
    pub step: Option<f64>,
    #[ts(type = "number")]
    pub use_count: i64,
    pub last_used_at: Option<String>,
    /// The shop it was last added at.
    pub shop_id: Option<String>,
    /// Hidden entries are never suggested.
    pub hidden: bool,
    #[sqlx(skip)]
    pub shops: Vec<CatalogShopUse>,
    pub created_at: String,
    pub updated_at: String,
    #[ts(type = "number")]
    pub rev: i64,
}
