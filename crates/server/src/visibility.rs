//! The single place that decides who can see and change shopping data (SPEC §7.3,
//! WORKPLAN A.3). Handlers never check ownership themselves: they ask here, and anything a
//! user may not see is a 404, so its existence never leaks.
//!
//! Phase 2: a list is visible to its owner only, and shops, staples and the catalogue are
//! personal. Phase 4 adds shares (users and households) by extending `VISIBLE_LISTS`,
//! `ROLE` and `list_role`; the rules for what each role allows live in
//! `shoppingcart_domain::visibility`.

use shoppingcart_domain::visibility::{self as rules};
pub use shoppingcart_domain::visibility::{Need, Role};
use sqlx::SqliteConnection;

use crate::{
    auth::AuthUser,
    error::{ApiResult, AppError},
};

/// SQL condition selecting the live lists (alias `l`) visible to the user bound as `?1`.
pub const VISIBLE_LISTS: &str = "(l.deleted_at IS NULL AND l.owner_id = ?1)";

/// SQL expression for the role (alias `l`) of the user bound as `?1` on a visible list.
pub const ROLE: &str = "(CASE WHEN l.owner_id = ?1 THEN 'owner' END)";

pub fn role_name(r: Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
    }
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
    Ok(owner.and_then(|o| rules::list_role(o == user_id, [])))
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
