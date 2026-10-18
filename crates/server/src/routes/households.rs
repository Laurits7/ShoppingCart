//! Households (SPEC §5.1, §6.12): groups of people to share with. Any user can create one
//! and becomes its owner; owners (and admins) rename it and manage members; anyone can leave.

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
    models::{Household, HouseholdMember},
    util::{id_or_new, name, now},
};

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
    rev: i64,
}

pub async fn load(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Option<Household>> {
    let Some(r) = sqlx::query_as::<_, Row>(
        "SELECT id, name, created_at, updated_at, rev FROM households WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let members = sqlx::query_as(
        "SELECT m.user_id, u.username, u.display_name, m.role FROM household_members m
         JOIN users u ON u.id = m.user_id
         WHERE m.household_id = ? AND u.deleted_at IS NULL
         ORDER BY m.role DESC, u.display_name",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Some(Household {
        id: r.id,
        name: r.name,
        members,
        created_at: r.created_at,
        updated_at: r.updated_at,
        rev: r.rev,
    }))
}

/// A household the user may see: as a member, or any household for admins. Others get 404.
async fn load_visible(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
) -> ApiResult<Household> {
    let h = load(conn, id).await?.ok_or(AppError::NotFound)?;
    if !user.user.is_admin && member(&h, user.id()).is_none() {
        return Err(AppError::NotFound);
    }
    Ok(h)
}

fn member<'a>(h: &'a Household, user_id: &str) -> Option<&'a HouseholdMember> {
    h.members.iter().find(|m| m.user_id == user_id)
}

fn require_owner(h: &Household, user: &AuthUser) -> ApiResult<()> {
    if user.user.is_admin || member(h, user.id()).is_some_and(|m| m.role == "owner") {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "only the household's owners can change it".into(),
        ))
    }
}

/// Every household keeps at least one owner: where none is left, the longest-standing
/// member becomes owner.
pub async fn ensure_owners(conn: &mut SqliteConnection) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE household_members SET role = 'owner'
         WHERE rowid IN (
           SELECT (SELECT m.rowid FROM household_members m WHERE m.household_id = h.household_id
                   ORDER BY m.added_at, m.user_id LIMIT 1)
           FROM (SELECT DISTINCT household_id FROM household_members) h
           WHERE NOT EXISTS (SELECT 1 FROM household_members o
                             WHERE o.household_id = h.household_id AND o.role = 'owner'))",
    )
    .execute(conn)
    .await?;
    Ok(())
}

async fn bump(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query("UPDATE households SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

fn check_role(role: &str) -> ApiResult<()> {
    if role == "owner" || role == "member" {
        Ok(())
    } else {
        Err(bad("role must be owner or member"))
    }
}

#[utoipa::path(get, path = "/households", tag = "households", summary = "Households you belong to (admins: all)", responses((status = 200, body = Vec<Household>), (status = 401, body = crate::error::Problem)))]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Household>>> {
    let mut conn = state.db.read.acquire().await?;
    let ids: Vec<String> = if user.user.is_admin {
        sqlx::query_scalar("SELECT id FROM households WHERE deleted_at IS NULL ORDER BY name")
            .fetch_all(&mut *conn)
            .await?
    } else {
        sqlx::query_scalar(
            "SELECT h.id FROM households h JOIN household_members m ON m.household_id = h.id
             WHERE m.user_id = ? AND h.deleted_at IS NULL ORDER BY h.name",
        )
        .bind(user.id())
        .fetch_all(&mut *conn)
        .await?
    };
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.extend(load(&mut conn, &id).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateHousehold {
    /// Optional client-chosen ULID.
    id: Option<String>,
    name: String,
}

#[utoipa::path(post, path = "/households", tag = "households", summary = "Create a household (you become its owner)", request_body = CreateHousehold, responses((status = 200, body = Household), (status = 400, body = crate::error::Problem)))]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(c): Json<CreateHousehold>,
) -> ApiResult<Json<Household>> {
    let id = id_or_new(c.id)?;
    let hname = name("name", &c.name, 100)?;
    let mut tx = state.db.write.begin().await?;
    // Retrying a create with the same id returns the household instead of failing.
    if let Some(existing) = load(&mut tx, &id).await? {
        return match member(&existing, user.id()) {
            Some(_) => Ok(Json(existing)),
            None => Err(AppError::Conflict("id already in use".into())),
        };
    }
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO households (id, name, created_by, created_at, updated_at, rev) VALUES (?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&hname)
    .bind(user.id())
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::Conflict("id already in use".into())
        }
        e => e.into(),
    })?;
    sqlx::query(
        "INSERT INTO household_members (household_id, user_id, role, added_at) VALUES (?,?,'owner',?)",
    )
    .bind(&id)
    .bind(user.id())
    .bind(&ts)
    .execute(&mut *tx)
    .await?;
    let h = load(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(h))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchHousehold {
    name: Option<String>,
}

#[utoipa::path(patch, path = "/households/{id}", tag = "households", summary = "Rename a household (owners)", params(("id" = String, Path, description = "ULID")), request_body = PatchHousehold, responses((status = 200, body = Household), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchHousehold>,
) -> ApiResult<Json<Household>> {
    let mut tx = state.db.write.begin().await?;
    let h = load_visible(&mut tx, &user, &id).await?;
    require_owner(&h, &user)?;
    if let Some(n) = p.name {
        sqlx::query("UPDATE households SET name = ? WHERE id = ?")
            .bind(name("name", &n, 100)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    bump(&mut tx, &id).await?;
    let h = load(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(h))
}

#[utoipa::path(delete, path = "/households/{id}", tag = "households", summary = "Delete a household (owners); ends all sharing through it", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    let h = load_visible(&mut tx, &user, &id).await?;
    require_owner(&h, &user)?;
    let members: Vec<String> = h.members.iter().map(|m| m.user_id.clone()).collect();
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query("UPDATE households SET deleted_at = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(&ts)
        .bind(&ts)
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM household_members WHERE household_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // Lists shared with the household are gone for its former members, also live.
    state.bus.users(rev, members);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddMember {
    user_id: String,
    /// `member` (default) or `owner`.
    role: Option<String>,
}

#[utoipa::path(post, path = "/households/{id}/members", tag = "households", summary = "Add a member (owners)", params(("id" = String, Path, description = "ULID")), request_body = AddMember, responses((status = 200, body = Household), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn add_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(a): Json<AddMember>,
) -> ApiResult<Json<Household>> {
    let role = a.role.as_deref().unwrap_or("member");
    check_role(role)?;
    let mut tx = state.db.write.begin().await?;
    let h = load_visible(&mut tx, &user, &id).await?;
    require_owner(&h, &user)?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL")
            .bind(&a.user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        return Err(bad("no such user"));
    }
    sqlx::query(
        "INSERT INTO household_members (household_id, user_id, role, added_at) VALUES (?,?,?,?)
         ON CONFLICT (household_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(&id)
    .bind(&a.user_id)
    .bind(role)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    bump(&mut tx, &id).await?;
    let h = load(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(h))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchMember {
    role: String,
}

#[utoipa::path(patch, path = "/households/{id}/members/{user_id}", tag = "households", summary = "Change a member's role (owners)", params(("id" = String, Path, description = "ULID"), ("user_id" = String, Path, description = "ULID")), request_body = PatchMember, responses((status = 200, body = Household), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, user_id)): Path<(String, String)>,
    Json(p): Json<PatchMember>,
) -> ApiResult<Json<Household>> {
    check_role(&p.role)?;
    let mut tx = state.db.write.begin().await?;
    let h = load_visible(&mut tx, &user, &id).await?;
    require_owner(&h, &user)?;
    if member(&h, &user_id).is_none() {
        return Err(AppError::NotFound);
    }
    let owners = h.members.iter().filter(|m| m.role == "owner").count();
    if p.role == "member" && owners == 1 && member(&h, &user_id).is_some_and(|m| m.role == "owner")
    {
        return Err(bad("a household needs at least one owner"));
    }
    sqlx::query("UPDATE household_members SET role = ? WHERE household_id = ? AND user_id = ?")
        .bind(&p.role)
        .bind(&id)
        .bind(&user_id)
        .execute(&mut *tx)
        .await?;
    bump(&mut tx, &id).await?;
    let h = load(&mut tx, &id).await?.ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(h))
}

/// Owners remove members; anyone can remove themselves (leave). Access through the
/// household ends immediately.
#[utoipa::path(delete, path = "/households/{id}/members/{user_id}", tag = "households", summary = "Remove a member, or leave", params(("id" = String, Path, description = "ULID"), ("user_id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, user_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    let h = load_visible(&mut tx, &user, &id).await?;
    if user_id != user.id() {
        require_owner(&h, &user)?;
    }
    if member(&h, &user_id).is_none() {
        return Err(AppError::NotFound);
    }
    sqlx::query("DELETE FROM household_members WHERE household_id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user_id)
        .execute(&mut *tx)
        .await?;
    ensure_owners(&mut tx).await?;
    bump(&mut tx, &id).await?;
    tx.commit().await?;
    // Lists shared with the household are gone for them at once, also live.
    state.bus.users(0, vec![user_id]);
    Ok(StatusCode::NO_CONTENT)
}
