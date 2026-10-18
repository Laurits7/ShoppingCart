//! The single place that decides who can see and change shopping data (SPEC §7.3,
//! WORKPLAN A.3). Handlers never check ownership themselves: they ask here, and anything a
//! user may not see is a 404, so its existence never leaks.
//!
//! A list is visible to its owner and to everyone it is shared with, directly or through a
//! household they are a member of now (SPEC §6.8). Shops, staples, the catalogue and products
//! are personal. What each role allows lives in `shoppingcart_domain::visibility`.

use shoppingcart_domain::visibility::{self as rules};
pub use shoppingcart_domain::visibility::{Need, Role, WishKind};
use sqlx::SqliteConnection;

use crate::{
    auth::AuthUser,
    error::{ApiResult, AppError},
};

/// Condition on a `shares s` row: a live share of list `list` reaching the user bound as
/// `?1`, directly or through a household they're in.
fn share_reaches(list: &str) -> String {
    shares_reaching("list", list)
}

/// The same for any kind of resource (`list` or `wishlist`).
fn shares_reaching(kind: &str, id: &str) -> String {
    format!(
        "s.resource_type = '{kind}' AND s.resource_id = {id} AND s.deleted_at IS NULL
         AND ((s.principal_type = 'user' AND s.principal_id = ?1)
           OR (s.principal_type = 'household' AND s.principal_id IN
                 (SELECT household_id FROM household_members WHERE user_id = ?1)))"
    )
}

/// SQL condition selecting the live lists (alias `l`) visible to the user bound as `?1`.
pub const VISIBLE_LISTS: &str = "(l.deleted_at IS NULL AND (l.owner_id = ?1 OR EXISTS (
     SELECT 1 FROM shares s
     WHERE s.resource_type = 'list' AND s.resource_id = l.id AND s.deleted_at IS NULL
       AND ((s.principal_type = 'user' AND s.principal_id = ?1)
         OR (s.principal_type = 'household' AND s.principal_id IN
               (SELECT household_id FROM household_members WHERE user_id = ?1))))))";

/// SQL expression for the role (alias `l`) of the user bound as `?1` on a visible list:
/// owner, else the strongest share.
pub fn role_sql() -> String {
    format!(
        "(CASE WHEN l.owner_id = ?1 THEN 'owner'
               WHEN EXISTS (SELECT 1 FROM shares s WHERE {} AND s.role = 'editor') THEN 'editor'
               ELSE 'viewer' END)",
        share_reaches("l.id")
    )
}

pub fn role_name(r: Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
    }
}

// ---- wishlists (SPEC §6.9, §7.3) -----------------------------------------------------------

/// SQL condition selecting the live wishlists (alias `w`) visible to the user bound as `?1`:
/// own or shared, never a gift list about them (whatever it's shared with).
pub const VISIBLE_WISHLISTS: &str = "(w.deleted_at IS NULL
     AND (w.owner_id = ?1 OR EXISTS (
         SELECT 1 FROM shares s
         WHERE s.resource_type = 'wishlist' AND s.resource_id = w.id AND s.deleted_at IS NULL
           AND ((s.principal_type = 'user' AND s.principal_id = ?1)
             OR (s.principal_type = 'household' AND s.principal_id IN
                   (SELECT household_id FROM household_members WHERE user_id = ?1)))))
     AND NOT (w.kind = 'gift' AND EXISTS (
         SELECT 1 FROM persons p WHERE p.id = w.recipient_person_id AND p.user_id = ?1)))";

/// SQL condition: visible wishlists (alias `w`) whose claims the user bound as `?1` sees (not
/// their own wishlist's).
pub const CLAIMS_VISIBLE: &str = "(w.deleted_at IS NULL
     AND (w.owner_id = ?1 OR EXISTS (
         SELECT 1 FROM shares s
         WHERE s.resource_type = 'wishlist' AND s.resource_id = w.id AND s.deleted_at IS NULL
           AND ((s.principal_type = 'user' AND s.principal_id = ?1)
             OR (s.principal_type = 'household' AND s.principal_id IN
                   (SELECT household_id FROM household_members WHERE user_id = ?1)))))
     AND NOT (w.kind = 'gift' AND EXISTS (
         SELECT 1 FROM persons p WHERE p.id = w.recipient_person_id AND p.user_id = ?1))
     AND NOT (w.kind = 'own' AND w.owner_id = ?1))";

/// SQL expression for the role (alias `w`) of the user bound as `?1` on a visible wishlist.
pub fn wish_role_sql() -> String {
    format!(
        "(CASE WHEN w.owner_id = ?1 THEN 'owner'
               WHEN EXISTS (SELECT 1 FROM shares s WHERE {} AND s.role = 'editor') THEN 'editor'
               ELSE 'viewer' END)",
        shares_reaching("wishlist", "w.id")
    )
}

/// What a user may do with a wishlist.
#[derive(Debug, Clone, Copy)]
pub struct WishAccess {
    pub role: Role,
    pub kind: WishKind,
    pub owner: bool,
}

impl WishAccess {
    pub fn sees_claims(&self) -> bool {
        rules::sees_claims(self.kind, self.owner)
    }
}

/// The user's access to a live wishlist, or `None` if they can't see it (a gift list about
/// them included).
pub async fn wishlist_access(
    conn: &mut SqliteConnection,
    user_id: &str,
    id: &str,
) -> sqlx::Result<Option<WishAccess>> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT w.owner_id, w.kind, p.user_id FROM wishlists w
         LEFT JOIN persons p ON p.id = w.recipient_person_id
         WHERE w.id = ? AND w.deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((owner, kind, recipient)) = row else {
        return Ok(None);
    };
    let kind = if kind == "gift" {
        WishKind::Gift
    } else {
        WishKind::Own
    };
    let roles: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT s.role FROM shares s WHERE {}",
        shares_reaching("wishlist", "?2")
    ))
    .bind(user_id)
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let owner = owner == user_id;
    Ok(rules::wishlist_role(
        kind,
        owner,
        recipient.as_deref() == Some(user_id),
        roles.iter().map(|r| {
            if r == "editor" {
                Role::Editor
            } else {
                Role::Viewer
            }
        }),
    )
    .map(|role| WishAccess { role, kind, owner }))
}

/// Access to a wishlist that allows `need`: 404 if it's not visible, 403 if it is but not
/// for this.
pub async fn require_wishlist(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
    need: Need,
) -> ApiResult<WishAccess> {
    let a = wishlist_access(conn, user.id(), id)
        .await?
        .ok_or(AppError::NotFound)?;
    if rules::allows(a.role, need) {
        Ok(a)
    } else {
        Err(AppError::Forbidden(match need {
            Need::Manage => "only the wishlist's owner can do this".into(),
            _ => "you can only view this wishlist".into(),
        }))
    }
}

/// The wishlist of a wish item, with the user's access for `need`. With `deleted`,
/// tombstoned items count too (for undo).
pub async fn require_wish_item(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
    need: Need,
    deleted: bool,
) -> ApiResult<(String, WishAccess)> {
    let w: String = sqlx::query_scalar(
        "SELECT wishlist_id FROM wish_items WHERE id = ? AND (deleted_at IS NULL OR ?)",
    )
    .bind(id)
    .bind(deleted)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    let a = require_wishlist(conn, user, &w, need).await?;
    Ok((w, a))
}

/// The wishlist of a live section, for `need`.
pub async fn require_wish_section(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
    need: Need,
) -> ApiResult<String> {
    let w: String = sqlx::query_scalar(
        "SELECT wishlist_id FROM wish_sections WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    require_wishlist(conn, user, &w, need).await?;
    Ok(w)
}

/// A live person of the user's own.
pub async fn require_person(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
) -> ApiResult<()> {
    let ok: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM persons WHERE id = ? AND owner_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user.id())
    .fetch_optional(&mut *conn)
    .await?;
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

/// The user's role on a live list, or `None` if they can't see it.
pub async fn list_role(
    conn: &mut SqliteConnection,
    user_id: &str,
    list_id: &str,
) -> sqlx::Result<Option<Role>> {
    let owner: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM lists WHERE id = ? AND deleted_at IS NULL")
            .bind(list_id)
            .fetch_optional(&mut *conn)
            .await?;
    let Some(owner) = owner else {
        return Ok(None);
    };
    let roles: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT s.role FROM shares s WHERE {}",
        share_reaches("?2")
    ))
    .bind(user_id)
    .bind(list_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rules::list_role(
        owner == user_id,
        roles.iter().map(|r| {
            if r == "editor" {
                Role::Editor
            } else {
                Role::Viewer
            }
        }),
    ))
}

/// The user's role on a list if it allows `need`. Lists they can't see are 404; lists they
/// can see but not change are 403.
pub async fn require_list(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    list_id: &str,
    need: Need,
) -> ApiResult<Role> {
    let role = list_role(conn, user.id(), list_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if rules::allows(role, need) {
        Ok(role)
    } else {
        Err(AppError::Forbidden(match need {
            Need::Manage => "only the list's owner can do this".into(),
            _ => "you can only view this list".into(),
        }))
    }
}

/// The list of an item the user may access for `need`. With `deleted`, tombstoned items
/// count too (for undo).
pub async fn require_item(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    item_id: &str,
    need: Need,
    deleted: bool,
) -> ApiResult<String> {
    let list_id: Option<String> =
        sqlx::query_scalar("SELECT list_id FROM items WHERE id = ? AND (deleted_at IS NULL OR ?)")
            .bind(item_id)
            .bind(deleted)
            .fetch_optional(&mut *conn)
            .await?;
    let list_id = list_id.ok_or(AppError::NotFound)?;
    require_list(conn, user, &list_id, need).await?;
    Ok(list_id)
}

/// The list of a live category the user may access for `need`.
pub async fn require_category(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    category_id: &str,
    need: Need,
) -> ApiResult<String> {
    let list_id: Option<String> =
        sqlx::query_scalar("SELECT list_id FROM categories WHERE id = ? AND deleted_at IS NULL")
            .bind(category_id)
            .fetch_optional(&mut *conn)
            .await?;
    let list_id = list_id.ok_or(AppError::NotFound)?;
    require_list(conn, user, &list_id, need).await?;
    Ok(list_id)
}

/// A live shop of the user's own (shops are personal).
pub async fn require_shop(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    shop_id: &str,
) -> ApiResult<()> {
    let ok: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM shops WHERE id = ? AND owner_id = ? AND deleted_at IS NULL",
    )
    .bind(shop_id)
    .bind(user.id())
    .fetch_optional(&mut *conn)
    .await?;
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

/// The shop of a live staple of one of the user's shops.
pub async fn require_staple(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    staple_id: &str,
) -> ApiResult<String> {
    let shop_id: Option<String> =
        sqlx::query_scalar("SELECT shop_id FROM staples WHERE id = ? AND deleted_at IS NULL")
            .bind(staple_id)
            .fetch_optional(&mut *conn)
            .await?;
    let shop_id = shop_id.ok_or(AppError::NotFound)?;
    require_shop(conn, user, &shop_id).await?;
    Ok(shop_id)
}

/// A live product of the user's own; returns its catalogue item.
pub async fn require_product(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    product_id: &str,
) -> ApiResult<String> {
    sqlx::query_scalar(
        "SELECT catalog_item_id FROM products WHERE id = ? AND user_id = ? AND deleted_at IS NULL",
    )
    .bind(product_id)
    .bind(user.id())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)
}

/// A live entry of the user's own catalogue.
pub async fn require_catalog_item(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
) -> ApiResult<()> {
    let ok: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM catalog_items WHERE id = ? AND user_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user.id())
    .fetch_optional(&mut *conn)
    .await?;
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

/// A picture the user may see: their own, or one shown by an item on a list they can see
/// (its pinned product's, its catalogue item's, or one of that item's products'), so
/// people on a shared list see the pictures of what's on it (D-21).
pub async fn require_image(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    image_id: &str,
) -> ApiResult<()> {
    let ok: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1 FROM images img WHERE img.id = ?2 AND (img.owner_id = ?1 OR EXISTS (
             SELECT 1 FROM items i JOIN lists l ON l.id = i.list_id
             WHERE {VISIBLE_LISTS} AND i.deleted_at IS NULL AND (
                 i.product_id IN (SELECT id FROM products WHERE image_id = img.id)
                 OR i.catalog_item_id IN (SELECT id FROM catalog_items WHERE image_id = img.id)
                 OR i.catalog_item_id IN (SELECT catalog_item_id FROM products
                                          WHERE image_id = img.id AND deleted_at IS NULL))))"
    ))
    .bind(user.id())
    .bind(image_id)
    .fetch_optional(&mut *conn)
    .await?;
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

/// A picture of the user's own, to attach to their products or catalogue items.
pub async fn require_own_image(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    image_id: &str,
) -> ApiResult<()> {
    let ok: Option<i64> = sqlx::query_scalar("SELECT 1 FROM images WHERE id = ? AND owner_id = ?")
        .bind(image_id)
        .bind(user.id())
        .fetch_optional(&mut *conn)
        .await?;
    ok.map(|_| ())
        .ok_or_else(|| crate::error::bad("no such picture"))
}
