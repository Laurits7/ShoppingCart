//! Wishlists, gift lists, their sections and items, persons and claims (SPEC §6.9). Who sees
//! what is decided in `visibility`: a gift list never reaches its recipient, and the owner of
//! an own wishlist never sees its claims (§7.3).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use shoppingcart_domain::{order, shopping::MAX_NAME_CHARS, shopping::MAX_NOTE_CHARS, wishes};
use sqlx::SqliteConnection;

use crate::{
    AppState,
    auth::AuthUser,
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::{Claim, Person, WishItem, WishSection, Wishlist},
    positions::{Place, Scope},
    util::{double_option, id_or_new, name, now},
    visibility::{self, CLAIMS_VISIBLE, Need, VISIBLE_WISHLISTS},
};

// ---- loading (also used by /sync) ---------------------------------------------------------

const PERSON_COLS: &str = "id, name, user_id, created_at, updated_at, rev";
const SECTION_COLS: &str =
    "x.id, x.wishlist_id, x.name, x.position, x.created_at, x.updated_at, x.rev";
const ITEM_COLS: &str = "x.id, x.wishlist_id, x.section_id, x.title, x.url, x.price, x.currency, x.note, x.priority, x.position, x.received_at, x.created_at, x.updated_at, x.rev";

pub async fn user_persons(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<Person>> {
    sqlx::query_as(&format!(
        "SELECT {PERSON_COLS} FROM persons WHERE owner_id = ? AND deleted_at IS NULL AND rev > ? ORDER BY name"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await
}

fn wishlist_select(cond: &str) -> String {
    let role = visibility::wish_role_sql();
    format!(
        "SELECT w.id, w.owner_id, u.display_name AS owner_name, w.kind, w.recipient_person_id,
                p.name AS recipient_name, w.title, w.position, {role} AS role,
                NOT (w.kind = 'own' AND w.owner_id = ?1) AS sees_claims,
                w.created_at, w.updated_at, w.rev
         FROM wishlists w JOIN users u ON u.id = w.owner_id
         LEFT JOIN persons p ON p.id = w.recipient_person_id
         WHERE {VISIBLE_WISHLISTS} AND {cond}
         ORDER BY w.position, w.id"
    )
}

pub async fn visible_wishlists(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<Wishlist>> {
    sqlx::query_as(&wishlist_select("w.rev > ?2"))
        .bind(uid)
        .bind(since)
        .fetch_all(conn)
        .await
}

async fn load_wishlist(conn: &mut SqliteConnection, uid: &str, id: &str) -> ApiResult<Wishlist> {
    sqlx::query_as(&wishlist_select("w.id = ?2"))
        .bind(uid)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn visible_wishlist_ids(
    conn: &mut SqliteConnection,
    uid: &str,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(&format!(
        "SELECT w.id FROM wishlists w WHERE {VISIBLE_WISHLISTS}"
    ))
    .bind(uid)
    .fetch_all(conn)
    .await
}

/// Wishlists whose claims the user sees (all visible ones but their own).
pub async fn claims_visible_ids(
    conn: &mut SqliteConnection,
    uid: &str,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(&format!(
        "SELECT w.id FROM wishlists w WHERE {CLAIMS_VISIBLE}"
    ))
    .bind(uid)
    .fetch_all(conn)
    .await
}

pub async fn visible_sections(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<WishSection>> {
    sqlx::query_as(&format!(
        "SELECT {SECTION_COLS} FROM wish_sections x JOIN wishlists w ON w.id = x.wishlist_id
         WHERE {VISIBLE_WISHLISTS} AND x.deleted_at IS NULL AND x.rev > ?2
         ORDER BY x.wishlist_id, x.position, x.id"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await
}

pub async fn visible_items(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<WishItem>> {
    sqlx::query_as(&format!(
        "SELECT {ITEM_COLS} FROM wish_items x JOIN wishlists w ON w.id = x.wishlist_id
         WHERE {VISIBLE_WISHLISTS} AND x.deleted_at IS NULL AND x.rev > ?2
         ORDER BY x.wishlist_id, x.position, x.id"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await
}

const CLAIM_SELECT: &str =
    "SELECT c.id, c.wish_item_id, c.user_id, u.display_name AS user_name, c.status, c.note,
        c.created_at, c.updated_at, c.rev
     FROM claims c JOIN users u ON u.id = c.user_id
     JOIN wish_items x ON x.id = c.wish_item_id JOIN wishlists w ON w.id = x.wishlist_id";

/// Live claims the user may see (never on their own wishlist).
pub async fn visible_claims(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<Claim>> {
    sqlx::query_as(&format!(
        "{CLAIM_SELECT} WHERE {CLAIMS_VISIBLE} AND c.deleted_at IS NULL AND x.deleted_at IS NULL AND c.rev > ?2"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await
}

/// Deleted wish things since `since`, for /sync: only of the user's own wishlists or ones
/// they see; released claims only where they see claims.
pub async fn deleted(
    conn: &mut SqliteConnection,
    uid: &str,
    since: i64,
) -> sqlx::Result<Vec<(String, String)>> {
    sqlx::query_as(&format!(
        "SELECT 'person', id FROM persons WHERE owner_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2
         UNION ALL SELECT 'wishlist', id FROM wishlists WHERE owner_id = ?1 AND deleted_at IS NOT NULL AND rev > ?2
         UNION ALL SELECT 'wish_section', x.id FROM wish_sections x JOIN wishlists w ON w.id = x.wishlist_id
               WHERE x.deleted_at IS NOT NULL AND x.rev > ?2 AND {VISIBLE_WISHLISTS}
         UNION ALL SELECT 'wish_item', x.id FROM wish_items x JOIN wishlists w ON w.id = x.wishlist_id
               WHERE x.deleted_at IS NOT NULL AND x.rev > ?2 AND {VISIBLE_WISHLISTS}
         UNION ALL SELECT 'claim', c.id FROM claims c JOIN wish_items x ON x.id = c.wish_item_id
               JOIN wishlists w ON w.id = x.wishlist_id
               WHERE c.deleted_at IS NOT NULL AND c.rev > ?2 AND {CLAIMS_VISIBLE}"
    ))
    .bind(uid)
    .bind(since)
    .fetch_all(conn)
    .await
}

async fn load_section(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<WishSection> {
    sqlx::query_as(&format!(
        "SELECT {SECTION_COLS} FROM wish_sections x WHERE x.id = ?"
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

async fn load_item(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<WishItem> {
    sqlx::query_as(&format!(
        "SELECT {ITEM_COLS} FROM wish_items x WHERE x.id = ?"
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

async fn load_claim(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Claim> {
    sqlx::query_as(&format!("{CLAIM_SELECT} WHERE c.id = ?"))
        .bind(id)
        .fetch_one(conn)
        .await
}

async fn bump(conn: &mut SqliteConnection, table: &str, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query(&format!(
        "UPDATE {table} SET updated_at = ?, rev = ? WHERE id = ?"
    ))
    .bind(now())
    .bind(rev)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

async fn tombstone(
    conn: &mut SqliteConnection,
    table: &str,
    cond: &str,
    id: &str,
) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query(&format!(
        "UPDATE {table} SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE deleted_at IS NULL AND {cond}"
    ))
    .bind(now())
    .bind(rev)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

fn note(s: &str) -> ApiResult<String> {
    let s = s.trim();
    if s.chars().count() > MAX_NOTE_CHARS {
        return Err(bad(format!(
            "note must be at most {MAX_NOTE_CHARS} characters"
        )));
    }
    Ok(s.to_string())
}

// ---- persons -------------------------------------------------------------------------------

#[utoipa::path(get, path = "/persons", tag = "wishlists", summary = "People you collect gift ideas for", responses((status = 200, body = Vec<Person>)))]
pub async fn list_persons(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Person>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(user_persons(&mut conn, user.id(), 0).await?))
}

/// A person's account must be someone else's (a gift list for yourself would hide from you).
async fn check_person_user(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    uid: &Option<String>,
) -> ApiResult<()> {
    if let Some(u) = uid {
        if u == user.id() {
            return Err(bad("that's you; gift ideas are for others"));
        }
        let ok: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL")
                .bind(u)
                .fetch_optional(conn)
                .await?;
        ok.ok_or_else(|| bad("no such user"))?;
    }
    Ok(())
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewPerson {
    id: Option<String>,
    name: String,
    /// Their account here, if they have one: gift lists about them are hidden from it.
    user_id: Option<String>,
}

#[utoipa::path(post, path = "/persons", tag = "wishlists", summary = "Add someone to collect gift ideas for", request_body = NewPerson, responses((status = 200, body = Person), (status = 400, body = crate::error::Problem)))]
pub async fn create_person(
    State(state): State<AppState>,
    user: AuthUser,
    Json(n): Json<NewPerson>,
) -> ApiResult<Json<Person>> {
    let id = id_or_new(n.id)?;
    let pname = name("name", &n.name, MAX_NAME_CHARS)?;
    let mut tx = state.db.write.begin().await?;
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM persons WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(o) = owner {
        return if o == user.id() {
            Ok(Json(
                sqlx::query_as(&format!("SELECT {PERSON_COLS} FROM persons WHERE id = ?"))
                    .bind(&id)
                    .fetch_one(&mut *tx)
                    .await?,
            ))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    check_person_user(&mut tx, &user, &n.user_id).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query("INSERT INTO persons (id, owner_id, name, user_id, created_at, updated_at, rev) VALUES (?,?,?,?,?,?,?)")
        .bind(&id)
        .bind(user.id())
        .bind(&pname)
        .bind(&n.user_id)
        .bind(&ts)
        .bind(&ts)
        .bind(rev)
        .execute(&mut *tx)
        .await
        .map_err(insert_err)?;
    let p = sqlx::query_as(&format!("SELECT {PERSON_COLS} FROM persons WHERE id = ?"))
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(p))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchPerson {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    user_id: Option<Option<String>>,
}

#[utoipa::path(patch, path = "/persons/{id}", tag = "wishlists", summary = "Rename a person or link their account", params(("id" = String, Path, description = "ULID")), request_body = PatchPerson, responses((status = 200, body = Person), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_person(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchPerson>,
) -> ApiResult<Json<Person>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_person(&mut tx, &user, &id).await?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE persons SET name = ? WHERE id = ?")
            .bind(name("name", &n, MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(u) = p.user_id {
        check_person_user(&mut tx, &user, &u).await?;
        sqlx::query("UPDATE persons SET user_id = ? WHERE id = ?")
            .bind(&u)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump(&mut tx, "persons", &id).await?;
    let person = sqlx::query_as(&format!("SELECT {PERSON_COLS} FROM persons WHERE id = ?"))
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(person))
}

#[utoipa::path(delete, path = "/persons/{id}", tag = "wishlists", summary = "Remove a person (only without gift lists)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem), (status = 409, description = "They still have gift lists", body = crate::error::Problem)))]
pub async fn delete_person(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_person(&mut tx, &user, &id).await?;
    let lists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wishlists WHERE recipient_person_id = ? AND deleted_at IS NULL",
    )
    .bind(&id)
    .fetch_one(&mut *tx)
    .await?;
    if lists > 0 {
        return Err(AppError::Conflict("delete their gift lists first".into()));
    }
    tombstone(&mut tx, "persons", "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- wishlists -------------------------------------------------------------------------------

#[utoipa::path(get, path = "/wishlists", tag = "wishlists", summary = "Wishlists and gift lists you can see", responses((status = 200, body = Vec<Wishlist>)))]
pub async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<Wishlist>>> {
    let mut conn = state.db.read.acquire().await?;
    Ok(Json(visible_wishlists(&mut conn, user.id(), 0).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewWishlist {
    id: Option<String>,
    /// `own` (your wishes) or `gift` (ideas for a person).
    kind: String,
    title: String,
    /// For `gift`: one of your persons.
    recipient_person_id: Option<String>,
}

#[utoipa::path(post, path = "/wishlists", tag = "wishlists", summary = "Create a wishlist (private until shared)", request_body = NewWishlist, responses((status = 200, body = Wishlist), (status = 400, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(n): Json<NewWishlist>,
) -> ApiResult<Json<Wishlist>> {
    let id = id_or_new(n.id)?;
    let title = name("title", &n.title, MAX_NAME_CHARS)?;
    let mut tx = state.db.write.begin().await?;
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_id FROM wishlists WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(o) = owner {
        return if o == user.id() {
            Ok(Json(load_wishlist(&mut tx, user.id(), &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    let recipient = match (n.kind.as_str(), n.recipient_person_id) {
        ("own", None) => None,
        ("gift", Some(p)) => {
            visibility::require_person(&mut tx, &user, &p)
                .await
                .map_err(|_| bad("no such person"))?;
            Some(p)
        }
        ("own", Some(_)) => return Err(bad("an own wishlist has no recipient")),
        ("gift", None) => return Err(bad("a gift list needs a recipient_person_id")),
        _ => return Err(bad("kind must be own or gift")),
    };
    let position = {
        let last: Option<String> =
            sqlx::query_scalar("SELECT MAX(position) FROM wishlists WHERE owner_id = ?")
                .bind(user.id())
                .fetch_one(&mut *tx)
                .await?;
        order::key_after(last.as_deref())
    };
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO wishlists (id, owner_id, kind, recipient_person_id, title, position, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(user.id())
    .bind(&n.kind)
    .bind(&recipient)
    .bind(&title)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    let w = load_wishlist(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(w))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchWishlist {
    title: Option<String>,
}

#[utoipa::path(patch, path = "/wishlists/{id}", tag = "wishlists", summary = "Rename a wishlist (owner)", params(("id" = String, Path, description = "ULID")), request_body = PatchWishlist, responses((status = 200, body = Wishlist), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchWishlist>,
) -> ApiResult<Json<Wishlist>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wishlist(&mut tx, &user, &id, Need::Manage).await?;
    if let Some(t) = p.title {
        sqlx::query("UPDATE wishlists SET title = ? WHERE id = ?")
            .bind(name("title", &t, MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump(&mut tx, "wishlists", &id).await?;
    let w = load_wishlist(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(w))
}

#[utoipa::path(delete, path = "/wishlists/{id}", tag = "wishlists", summary = "Delete a wishlist with its items (owner)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wishlist(&mut tx, &user, &id, Need::Manage).await?;
    tombstone(&mut tx, "wish_items", "wishlist_id = ?3", &id).await?;
    tombstone(&mut tx, "wish_sections", "wishlist_id = ?3", &id).await?;
    tombstone(
        &mut tx,
        "shares",
        "resource_type = 'wishlist' AND resource_id = ?3",
        &id,
    )
    .await?;
    tombstone(&mut tx, "wishlists", "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/wishlists/{id}/move", tag = "wishlists", summary = "Reorder your wishlists", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = Wishlist), (status = 404, body = crate::error::Problem)))]
pub async fn move_wishlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<Wishlist>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wishlist(&mut tx, &user, &id, Need::Manage).await?;
    let scope = Scope {
        table: "wishlists",
        column: "owner_id",
        id: user.id(),
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    sqlx::query("UPDATE wishlists SET position = ? WHERE id = ?")
        .bind(key)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    bump(&mut tx, "wishlists", &id).await?;
    let w = load_wishlist(&mut tx, user.id(), &id).await?;
    tx.commit().await?;
    Ok(Json(w))
}

// ---- sections --------------------------------------------------------------------------

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewSection {
    id: Option<String>,
    name: String,
}

#[utoipa::path(post, path = "/wishlists/{id}/sections", tag = "wishlists", summary = "Add a topic section", params(("id" = String, Path, description = "Wishlist ULID")), request_body = NewSection, responses((status = 200, body = WishSection), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_section(
    State(state): State<AppState>,
    user: AuthUser,
    Path(wid): Path<String>,
    Json(n): Json<NewSection>,
) -> ApiResult<Json<WishSection>> {
    let id = id_or_new(n.id)?;
    let sname = name("name", &n.name, MAX_NAME_CHARS)?;
    let mut tx = state.db.write.begin().await?;
    visibility::require_wishlist(&mut tx, &user, &wid, Need::EditItems).await?;
    let existing: Option<String> =
        sqlx::query_scalar("SELECT wishlist_id FROM wish_sections WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(w) = existing {
        return if w == wid {
            Ok(Json(load_section(&mut tx, &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    let position = next_position(&mut tx, "wish_sections", "wishlist_id", &wid).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query("INSERT INTO wish_sections (id, wishlist_id, name, position, created_at, updated_at, rev) VALUES (?,?,?,?,?,?,?)")
        .bind(&id)
        .bind(&wid)
        .bind(&sname)
        .bind(&position)
        .bind(&ts)
        .bind(&ts)
        .bind(rev)
        .execute(&mut *tx)
        .await
        .map_err(insert_err)?;
    let s = load_section(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(s))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchSection {
    name: Option<String>,
}

#[utoipa::path(patch, path = "/wish-sections/{id}", tag = "wishlists", summary = "Rename a section", params(("id" = String, Path, description = "ULID")), request_body = PatchSection, responses((status = 200, body = WishSection), (status = 404, body = crate::error::Problem)))]
pub async fn patch_section(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchSection>,
) -> ApiResult<Json<WishSection>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wish_section(&mut tx, &user, &id, Need::EditItems).await?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE wish_sections SET name = ? WHERE id = ?")
            .bind(name("name", &n, MAX_NAME_CHARS)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump(&mut tx, "wish_sections", &id).await?;
    let s = load_section(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(s))
}

#[utoipa::path(delete, path = "/wish-sections/{id}", tag = "wishlists", summary = "Delete a section; its items stay, without a section", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete_section(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wish_section(&mut tx, &user, &id, Need::EditItems).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "UPDATE wish_items SET section_id = NULL, updated_at = ?1, rev = ?2 WHERE section_id = ?3",
    )
    .bind(now())
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    tombstone(&mut tx, "wish_sections", "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/wish-sections/{id}/move", tag = "wishlists", summary = "Reorder a section", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = WishSection), (status = 404, body = crate::error::Problem)))]
pub async fn move_section(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<WishSection>> {
    let mut tx = state.db.write.begin().await?;
    let wid = visibility::require_wish_section(&mut tx, &user, &id, Need::EditItems).await?;
    let scope = Scope {
        table: "wish_sections",
        column: "wishlist_id",
        id: &wid,
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    sqlx::query("UPDATE wish_sections SET position = ? WHERE id = ?")
        .bind(key)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    bump(&mut tx, "wish_sections", &id).await?;
    let s = load_section(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(s))
}

// ---- items -------------------------------------------------------------------------------

/// The section, if given, must be a live one of the wishlist.
async fn check_section(
    conn: &mut SqliteConnection,
    wid: &str,
    sid: &Option<String>,
) -> ApiResult<()> {
    if let Some(s) = sid {
        let ok: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM wish_sections WHERE id = ? AND wishlist_id = ? AND deleted_at IS NULL",
        )
        .bind(s)
        .bind(wid)
        .fetch_optional(conn)
        .await?;
        ok.ok_or_else(|| bad("that section isn't on this wishlist"))?;
    }
    Ok(())
}

/// The user's currency preference (D-18), EUR by default.
fn default_currency(user: &AuthUser) -> String {
    serde_json::from_str::<serde_json::Value>(&user.user.prefs)
        .ok()
        .and_then(|p| p.get("currency").and_then(|c| c.as_str()).map(String::from))
        .and_then(|c| wishes::check_currency(&c).ok())
        .unwrap_or_else(|| "EUR".into())
}

#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct NewWishItem {
    id: Option<String>,
    title: String,
    url: Option<String>,
    price: Option<f64>,
    /// Default: your currency setting (EUR).
    currency: Option<String>,
    note: Option<String>,
    /// 0 normal, 1 would like a lot, 2 most wanted.
    priority: Option<i64>,
    section_id: Option<String>,
}

#[utoipa::path(post, path = "/wishlists/{id}/items", tag = "wishlists", summary = "Add a wish", params(("id" = String, Path, description = "Wishlist ULID")), request_body = NewWishItem, responses((status = 200, body = WishItem), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(wid): Path<String>,
    Json(n): Json<NewWishItem>,
) -> ApiResult<Json<WishItem>> {
    let id = id_or_new(n.id)?;
    let title = name("title", &n.title, MAX_NAME_CHARS)?;
    let url = wishes::check_url(n.url.as_deref().unwrap_or("")).map_err(bad)?;
    let price = n.price.map(wishes::check_price).transpose().map_err(bad)?;
    let currency = match (&price, n.currency.as_deref()) {
        (Some(_), Some(c)) => Some(wishes::check_currency(c).map_err(bad)?),
        (Some(_), None) => Some(default_currency(&user)),
        (None, _) => None,
    };
    let priority = wishes::check_priority(n.priority.unwrap_or(0)).map_err(bad)?;
    let wnote = note(n.note.as_deref().unwrap_or(""))?;
    let mut tx = state.db.write.begin().await?;
    visibility::require_wishlist(&mut tx, &user, &wid, Need::EditItems).await?;
    let existing: Option<String> =
        sqlx::query_scalar("SELECT wishlist_id FROM wish_items WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(w) = existing {
        return if w == wid {
            Ok(Json(load_item(&mut tx, &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    check_section(&mut tx, &wid, &n.section_id).await?;
    let position = next_position(&mut tx, "wish_items", "wishlist_id", &wid).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO wish_items (id, wishlist_id, section_id, title, url, price, currency, note, priority, position,
                                 created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&wid)
    .bind(&n.section_id)
    .bind(&title)
    .bind(&url)
    .bind(price)
    .bind(&currency)
    .bind(&wnote)
    .bind(priority)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    let item = load_item(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(item))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchWishItem {
    title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<f64>)]
    price: Option<Option<f64>>,
    currency: Option<String>,
    note: Option<String>,
    priority: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    section_id: Option<Option<String>>,
    /// `true` moves it to the Received archive, `false` brings it back.
    received: Option<bool>,
}

#[utoipa::path(patch, path = "/wish-items/{id}", tag = "wishlists", summary = "Edit a wish, or mark it received", params(("id" = String, Path, description = "ULID")), request_body = PatchWishItem, responses((status = 200, body = WishItem), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchWishItem>,
) -> ApiResult<Json<WishItem>> {
    let mut tx = state.db.write.begin().await?;
    let (wid, _) =
        visibility::require_wish_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    let cur = load_item(&mut tx, &id).await?;
    let title = match p.title {
        Some(t) => name("title", &t, MAX_NAME_CHARS)?,
        None => cur.title,
    };
    let url = match p.url {
        Some(u) => wishes::check_url(u.as_deref().unwrap_or("")).map_err(bad)?,
        None => cur.url,
    };
    let price = match p.price {
        Some(v) => v.map(wishes::check_price).transpose().map_err(bad)?,
        None => cur.price,
    };
    let currency = match (&price, p.currency) {
        (None, _) => None,
        (Some(_), Some(c)) => Some(wishes::check_currency(&c).map_err(bad)?),
        (Some(_), None) => cur.currency.or_else(|| Some(default_currency(&user))),
    };
    let priority = match p.priority {
        Some(v) => wishes::check_priority(v).map_err(bad)?,
        None => cur.priority,
    };
    let wnote = match p.note {
        Some(n) => note(&n)?,
        None => cur.note,
    };
    let section = match p.section_id {
        Some(s) => {
            check_section(&mut tx, &wid, &s).await?;
            s
        }
        None => cur.section_id,
    };
    let received = match p.received {
        Some(true) => Some(cur.received_at.unwrap_or_else(now)),
        Some(false) => None,
        None => cur.received_at,
    };
    let rev = next_rev(&mut tx).await?;
    sqlx::query(
        "UPDATE wish_items SET title = ?, url = ?, price = ?, currency = ?, priority = ?, note = ?, section_id = ?,
                               received_at = ?, updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&title)
    .bind(&url)
    .bind(price)
    .bind(&currency)
    .bind(priority)
    .bind(&wnote)
    .bind(&section)
    .bind(&received)
    .bind(now())
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    let item = load_item(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(item))
}

#[utoipa::path(delete, path = "/wish-items/{id}", tag = "wishlists", summary = "Delete a wish (restore to undo)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_wish_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    tombstone(&mut tx, "wish_items", "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct WishIds {
    ids: Vec<String>,
}

#[utoipa::path(post, path = "/wish-items/restore", tag = "wishlists", summary = "Undo: bring deleted wishes back", request_body = WishIds, responses((status = 200, body = Vec<WishItem>)))]
pub async fn restore_items(
    State(state): State<AppState>,
    user: AuthUser,
    Json(r): Json<WishIds>,
) -> ApiResult<Json<Vec<WishItem>>> {
    if r.ids.len() > 500 {
        return Err(bad("at most 500 at once"));
    }
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let mut out = vec![];
    for id in &r.ids {
        if visibility::require_wish_item(&mut tx, &user, id, Need::EditItems, true)
            .await
            .is_err()
        {
            continue;
        }
        sqlx::query(
            "UPDATE wish_items SET deleted_at = NULL, updated_at = ?1, rev = ?2,
                 section_id = (SELECT s.id FROM wish_sections s WHERE s.id = wish_items.section_id AND s.deleted_at IS NULL)
             WHERE id = ?3",
        )
        .bind(now())
        .bind(rev)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        out.push(load_item(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(Json(out))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MoveWishItem {
    after: Option<String>,
    before: Option<String>,
    /// Another section of the wishlist, or null for none. Absent: unchanged.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    section_id: Option<Option<String>>,
}

#[utoipa::path(post, path = "/wish-items/{id}/move", tag = "wishlists", summary = "Reorder a wish or move it to another section", params(("id" = String, Path, description = "ULID")), request_body = MoveWishItem, responses((status = 200, body = WishItem), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(m): Json<MoveWishItem>,
) -> ApiResult<Json<WishItem>> {
    let mut tx = state.db.write.begin().await?;
    let (wid, _) =
        visibility::require_wish_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    if let Some(s) = &m.section_id {
        check_section(&mut tx, &wid, s).await?;
        sqlx::query("UPDATE wish_items SET section_id = ? WHERE id = ?")
            .bind(s)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    let scope = Scope {
        table: "wish_items",
        column: "wishlist_id",
        id: &wid,
    };
    let key = scope
        .key_for(
            &mut tx,
            &id,
            &Place {
                after: m.after,
                before: m.before,
            },
        )
        .await?;
    sqlx::query("UPDATE wish_items SET position = ? WHERE id = ?")
        .bind(key)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    bump(&mut tx, "wish_items", &id).await?;
    let item = load_item(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(item))
}

// ---- claims ----------------------------------------------------------------------------------

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewClaim {
    /// Optional client-chosen ULID.
    id: Option<String>,
    note: Option<String>,
}

#[utoipa::path(post, path = "/wish-items/{id}/claim", tag = "wishlists", summary = "\"I'll get this\"", params(("id" = String, Path, description = "Wish item ULID")), request_body = NewClaim, responses((status = 200, body = Claim), (status = 403, description = "Your own wishlist", body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, description = "Someone else is getting it", body = crate::error::Problem)))]
pub async fn claim(
    State(state): State<AppState>,
    user: AuthUser,
    Path(item): Path<String>,
    Json(n): Json<NewClaim>,
) -> ApiResult<Json<Claim>> {
    let id = id_or_new(n.id)?;
    let cnote = note(n.note.as_deref().unwrap_or(""))?;
    let mut tx = state.db.write.begin().await?;
    let (_, access) =
        visibility::require_wish_item(&mut tx, &user, &item, Need::Read, false).await?;
    if !access.sees_claims() {
        return Err(AppError::Forbidden(
            "you can't claim things on your own wishlist".into(),
        ));
    }
    let live: Option<(String, String)> = sqlx::query_as(
        "SELECT id, user_id FROM claims WHERE wish_item_id = ? AND deleted_at IS NULL",
    )
    .bind(&item)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((cid, by)) = live {
        return if by == user.id() {
            Ok(Json(load_claim(&mut tx, &cid).await?))
        } else {
            Err(AppError::Conflict(
                "someone else is already getting this".into(),
            ))
        };
    }
    // Claims have their own revs and never touch the item, so its owner can't notice them.
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO claims (id, wish_item_id, user_id, status, note, created_at, updated_at, rev)
         VALUES (?,?,?,'planned',?,?,?,?)",
    )
    .bind(&id)
    .bind(&item)
    .bind(user.id())
    .bind(&cnote)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    let c = load_claim(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(c))
}

/// A live claim of the user's own (others' claims: 403 if they can see it, else 404).
async fn own_claim(conn: &mut SqliteConnection, user: &AuthUser, id: &str) -> ApiResult<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT wish_item_id, user_id FROM claims WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let (item, by) = row.ok_or(AppError::NotFound)?;
    let (_, access) = visibility::require_wish_item(conn, user, &item, Need::Read, true).await?;
    if !access.sees_claims() {
        return Err(AppError::NotFound);
    }
    if by != user.id() {
        return Err(AppError::Forbidden("that's someone else's claim".into()));
    }
    Ok(())
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchClaim {
    /// `planned` or `bought`.
    status: Option<String>,
    note: Option<String>,
}

#[utoipa::path(patch, path = "/claims/{id}", tag = "wishlists", summary = "Mark your claim bought (or planned again), or change its note", params(("id" = String, Path, description = "ULID")), request_body = PatchClaim, responses((status = 200, body = Claim), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_claim(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchClaim>,
) -> ApiResult<Json<Claim>> {
    let mut tx = state.db.write.begin().await?;
    own_claim(&mut tx, &user, &id).await?;
    if let Some(s) = p.status {
        if s != "planned" && s != "bought" {
            return Err(bad("status must be planned or bought"));
        }
        sqlx::query("UPDATE claims SET status = ? WHERE id = ?")
            .bind(&s)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(n) = p.note {
        sqlx::query("UPDATE claims SET note = ? WHERE id = ?")
            .bind(note(&n)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump(&mut tx, "claims", &id).await?;
    let c = load_claim(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(c))
}

#[utoipa::path(delete, path = "/claims/{id}", tag = "wishlists", summary = "Release your claim", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn release(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    own_claim(&mut tx, &user, &id).await?;
    tombstone(&mut tx, "claims", "id = ?3", &id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
