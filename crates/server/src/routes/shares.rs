//! Sharing lists with people and households (SPEC §6.8, D-8): only the owner shares,
//! re-shares and revokes; someone a list is shared with directly can leave it.

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

const SELECT: &str = "SELECT s.id, s.resource_id AS list_id, s.principal_type, s.principal_id,
        COALESCE(CASE s.principal_type WHEN 'user' THEN u.display_name ELSE h.name END, '') AS name,
        s.role, s.created_at, s.rev
     FROM shares s
     LEFT JOIN users u ON s.principal_type = 'user' AND u.id = s.principal_id
     LEFT JOIN households h ON s.principal_type = 'household' AND h.id = s.principal_id
     WHERE s.resource_type = 'list' AND s.deleted_at IS NULL";

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

#[utoipa::path(get, path = "/lists/{id}/shares", tag = "sharing", summary = "Who a list is shared with", params(("id" = String, Path, description = "List ULID")), responses((status = 200, body = Vec<Share>), (status = 404, body = crate::error::Problem)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
) -> ApiResult<Json<Vec<Share>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_list(&mut conn, &user, &list_id, Need::Read).await?;
    let shares = sqlx::query_as(&format!(
        "{SELECT} AND s.resource_id = ? ORDER BY s.principal_type DESC, name"
    ))
    .bind(&list_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(with_members(&mut conn, shares).await?))
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
    Path(list_id): Path<String>,
    Json(n): Json<NewShare>,
) -> ApiResult<Json<Share>> {
    check_role(&n.role)?;
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::Manage).await?;
    match n.principal_type.as_str() {
        "user" => {
            if n.principal_id == user.id() {
                return Err(bad("you already own this list"));
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
         VALUES (?1, 'list', ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)
         ON CONFLICT (resource_type, resource_id, principal_type, principal_id) WHERE deleted_at IS NULL
         DO UPDATE SET role = excluded.role, updated_at = excluded.updated_at, rev = excluded.rev
         RETURNING id",
    )
    .bind(new_id())
    .bind(&list_id)
    .bind(&n.principal_type)
    .bind(&n.principal_id)
    .bind(&n.role)
    .bind(user.id())
    .bind(&ts)
    .bind(rev)
    .fetch_one(&mut *tx)
    .await?;
    let share = load(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(share))
}

/// A live share of a list, with the list.
async fn find(conn: &mut SqliteConnection, id: &str) -> ApiResult<(String, String, String)> {
    sqlx::query_as(
        "SELECT resource_id, principal_type, principal_id FROM shares
         WHERE id = ? AND resource_type = 'list' AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound)
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
    let (list_id, _, _) = find(&mut tx, &id).await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::Manage).await?;
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

#[utoipa::path(delete, path = "/shares/{id}", tag = "sharing", summary = "Stop sharing (owner), or leave a list shared with you directly", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done; access ends at once, also on open connections"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    let (list_id, ptype, pid) = find(&mut tx, &id).await?;
    let leaving = ptype == "user" && pid == user.id();
    if !leaving {
        visibility::require_list(&mut tx, &user, &list_id, Need::Manage).await?;
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
