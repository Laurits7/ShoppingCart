//! Sharing lists and wishlists with people and households (SPEC §6.8, §6.9, D-8): only the
//! owner shares, re-shares and revokes; someone it's shared with directly can leave it. A gift
//! list is never shared with the person it's for (and hidden from them anyway).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use sqlx::SqliteConnection;

use crate::{
    AppState,
    auth::AuthUser,
    db::next_rev,
    error::{ApiResult, AppError, bad},
    models::Share,
    util::{new_id, now},
    visibility::{self, Need},
};

const SELECT: &str =
    "SELECT s.id, s.resource_type, s.resource_id, s.principal_type, s.principal_id,
        COALESCE(CASE s.principal_type WHEN 'user' THEN u.display_name ELSE h.name END, '') AS name,
        s.role, s.created_at, s.rev
     FROM shares s
     LEFT JOIN users u ON s.principal_type = 'user' AND u.id = s.principal_id
     LEFT JOIN households h ON s.principal_type = 'household' AND h.id = s.principal_id
     WHERE s.deleted_at IS NULL";

async fn with_members(
    conn: &mut SqliteConnection,
    mut shares: Vec<Share>,
) -> sqlx::Result<Vec<Share>> {
    for s in shares
        .iter_mut()
        .filter(|s| s.principal_type == "household")
    {
        s.members = sqlx::query_scalar(
            "SELECT u.display_name FROM household_members m JOIN users u ON u.id = m.user_id
             WHERE m.household_id = ? AND u.deleted_at IS NULL ORDER BY u.display_name",
        )
        .bind(&s.principal_id)
        .fetch_all(&mut *conn)
        .await?;
    }
    Ok(shares)
}

async fn load(conn: &mut SqliteConnection, id: &str) -> ApiResult<Share> {
    let s: Share = sqlx::query_as(&format!("{SELECT} AND s.id = ?"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(with_members(conn, vec![s]).await?.remove(0))
}

/// A list or a wishlist.
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    List,
    Wishlist,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::List => "list",
            Kind::Wishlist => "wishlist",
        }
    }
}

/// The user may do `need` with the resource (404/403 otherwise).
async fn require(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    kind: Kind,
    id: &str,
    need: Need,
) -> ApiResult<()> {
    match kind {
        Kind::List => visibility::require_list(conn, user, id, need)
            .await
            .map(|_| ()),
        Kind::Wishlist => visibility::require_wishlist(conn, user, id, need)
            .await
            .map(|_| ()),
    }
}

async fn list_of(state: &AppState, user: &AuthUser, kind: Kind, id: &str) -> ApiResult<Vec<Share>> {
    let mut conn = state.db.read.acquire().await?;
    require(&mut conn, user, kind, id, Need::Read).await?;
    let shares = sqlx::query_as(&format!(
        "{SELECT} AND s.resource_type = ? AND s.resource_id = ? ORDER BY s.principal_type DESC, name"
    ))
    .bind(kind.name())
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(with_members(&mut conn, shares).await?)
}

#[utoipa::path(get, path = "/lists/{id}/shares", tag = "sharing", summary = "Who a list is shared with", params(("id" = String, Path, description = "List ULID")), responses((status = 200, body = Vec<Share>), (status = 404, body = crate::error::Problem)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Share>>> {
    Ok(Json(list_of(&state, &user, Kind::List, &id).await?))
}

#[utoipa::path(get, path = "/wishlists/{id}/shares", tag = "sharing", summary = "Who a wishlist is shared with", params(("id" = String, Path, description = "Wishlist ULID")), responses((status = 200, body = Vec<Share>), (status = 404, body = crate::error::Problem)))]
pub async fn list_wishlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Share>>> {
    Ok(Json(list_of(&state, &user, Kind::Wishlist, &id).await?))
}

fn check_role(r: &str) -> ApiResult<()> {
    if r == "editor" || r == "viewer" {
        Ok(())
    } else {
        Err(bad("role must be editor or viewer"))
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct NewShare {
    /// `user` or `household`.
    principal_type: String,
    principal_id: String,
    /// `editor` (add, tick, edit items and categories) or `viewer` (read only).
    role: String,
}

#[utoipa::path(post, path = "/lists/{id}/shares", tag = "sharing", summary = "Share a list with a person or a household you're in (owner); sharing again changes the role", params(("id" = String, Path, description = "List ULID")), request_body = NewShare, responses((status = 200, body = Share), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(n): Json<NewShare>,
) -> ApiResult<Json<Share>> {
    Ok(Json(share(&state, &user, Kind::List, &id, n).await?))
}

#[utoipa::path(post, path = "/wishlists/{id}/shares", tag = "sharing", summary = "Share a wishlist with a person or a household you're in (owner); a gift list is never shown to its recipient", params(("id" = String, Path, description = "Wishlist ULID")), request_body = NewShare, responses((status = 200, body = Share), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_wishlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(n): Json<NewShare>,
) -> ApiResult<Json<Share>> {
    Ok(Json(share(&state, &user, Kind::Wishlist, &id, n).await?))
}

async fn share(
    state: &AppState,
    user: &AuthUser,
    kind: Kind,
    list_id: &str,
    n: NewShare,
) -> ApiResult<Share> {
    check_role(&n.role)?;
    let mut tx = state.db.write.begin().await?;
    require(&mut tx, user, kind, list_id, Need::Manage).await?;
    match n.principal_type.as_str() {
        "user" => {
            if n.principal_id == user.id() {
                return Err(bad("you already own it"));
            }
            if kind == Kind::Wishlist {
                let recipient: Option<Option<String>> = sqlx::query_scalar(
                    "SELECT p.user_id FROM wishlists w JOIN persons p ON p.id = w.recipient_person_id WHERE w.id = ?",
                )
                .bind(list_id)
                .fetch_optional(&mut *tx)
                .await?;
                if recipient.flatten().as_deref() == Some(n.principal_id.as_str()) {
                    return Err(bad("this gift list is for them; it stays hidden from them"));
                }
            }
            let ok: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL")
                    .bind(&n.principal_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            ok.ok_or_else(|| bad("no such user"))?;
        }
        "household" => {
            // Only households you're in, so nobody can push lists into other households.
            let ok: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM household_members m JOIN households h ON h.id = m.household_id
                 WHERE m.household_id = ? AND m.user_id = ? AND h.deleted_at IS NULL",
            )
            .bind(&n.principal_id)
            .bind(user.id())
            .fetch_optional(&mut *tx)
            .await?;
            ok.ok_or_else(|| bad("you can only share with households you're in"))?;
        }
        _ => return Err(bad("principal_type must be user or household")),
    }
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    let id: String = sqlx::query_scalar(
        "INSERT INTO shares (id, resource_type, resource_id, principal_type, principal_id, role, created_by,
                             created_at, updated_at, rev)
         VALUES (?1, ?9, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)
         ON CONFLICT (resource_type, resource_id, principal_type, principal_id) WHERE deleted_at IS NULL
         DO UPDATE SET role = excluded.role, updated_at = excluded.updated_at, rev = excluded.rev
         RETURNING id",
    )
    .bind(new_id())
    .bind(list_id)
    .bind(&n.principal_type)
    .bind(&n.principal_id)
    .bind(&n.role)
    .bind(user.id())
    .bind(&ts)
    .bind(rev)
    .bind(kind.name())
    .fetch_one(&mut *tx)
    .await?;
    let share = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(share)
}

/// A live share: what it shares, and with whom.
async fn find(conn: &mut SqliteConnection, id: &str) -> ApiResult<(Kind, String, String, String)> {
    let (kind, rid, ptype, pid): (String, String, String, String) = sqlx::query_as(
        "SELECT resource_type, resource_id, principal_type, principal_id FROM shares
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound)?;
    let kind = if kind == "wishlist" {
        Kind::Wishlist
    } else {
        Kind::List
    };
    Ok((kind, rid, ptype, pid))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchShare {
    role: String,
}

#[utoipa::path(patch, path = "/shares/{id}", tag = "sharing", summary = "Change a share's role (owner)", params(("id" = String, Path, description = "ULID")), request_body = PatchShare, responses((status = 200, body = Share), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchShare>,
) -> ApiResult<Json<Share>> {
    check_role(&p.role)?;
    let mut tx = state.db.write.begin().await?;
    let (kind, rid, _, _) = find(&mut tx, &id).await?;
    require(&mut tx, &user, kind, &rid, Need::Manage).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE shares SET role = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(&p.role)
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let share = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(share))
}

#[utoipa::path(delete, path = "/shares/{id}", tag = "sharing", summary = "Stop sharing (owner), or leave something shared with you directly", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done; access ends at once, also on open connections"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    let (kind, rid, ptype, pid) = find(&mut tx, &id).await?;
    let leaving = ptype == "user" && pid == user.id();
    if !leaving {
        require(&mut tx, &user, kind, &rid, Need::Manage).await?;
    }
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query("UPDATE shares SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
        .bind(&ts)
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
