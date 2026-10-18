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
