//! Categories and items (SPEC §6.4): quick add with the duplicate guard, ticking, the −/+
//! stepper, finish shopping and undo, adding staples to a list.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use shoppingcart_domain::{
    amount,
    catalog::normalize_name,
    shopping::{MAX_NAME_CHARS, MAX_NOTE_CHARS},
};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::catalog::{self, Use};
use crate::{
    AppState,
    auth::{AuthUser, Credential},
    db::{next_position, next_rev},
    error::{ApiResult, AppError, bad, insert_err},
    models::{Category, Item},
    positions::{Place, Scope},
    util::{double_option, id_or_new, name, now},
    visibility::{self, Need, VISIBLE_LISTS},
};

const CAT_COLS: &[&str] = &[
    "id",
    "list_id",
    "name",
    "position",
    "created_at",
    "updated_at",
    "rev",
];
const ITEM_COLS: &[&str] = &[
    "id",
    "list_id",
    "category_id",
    "catalog_item_id",
    "name",
    "amount",
    "unit",
    "step",
    "note",
    "position",
    "checked_at",
    "checked_by",
    "added_by",
    "source",
    "external_ref",
    "created_at",
    "updated_at",
    "rev",
];

fn cols(prefix: &str, cols: &[&str]) -> String {
    cols.iter()
        .map(|c| format!("{prefix}{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Live categories of the lists visible to `user_id`, optionally of one shop only.
pub async fn visible_categories(
    conn: &mut SqliteConnection,
    user_id: &str,
    shop_id: Option<&str>,
) -> sqlx::Result<Vec<Category>> {
    sqlx::query_as(&format!(
        "SELECT {} FROM categories c JOIN lists l ON l.id = c.list_id
         WHERE {VISIBLE_LISTS} AND c.deleted_at IS NULL AND (?2 IS NULL OR l.shop_id = ?2)
         ORDER BY c.list_id, c.position, c.id",
        cols("c.", CAT_COLS)
    ))
    .bind(user_id)
    .bind(shop_id)
    .fetch_all(conn)
    .await
}

/// Live categories of one list (the caller has checked access).
pub async fn list_categories(
    conn: &mut SqliteConnection,
    list_id: &str,
) -> sqlx::Result<Vec<Category>> {
    sqlx::query_as(&format!(
        "SELECT {} FROM categories WHERE list_id = ? AND deleted_at IS NULL ORDER BY position, id",
        cols("", CAT_COLS)
    ))
    .bind(list_id)
    .fetch_all(conn)
    .await
}

/// Live items of the lists visible to `user_id`, optionally of one list only.
pub async fn visible_items(
    conn: &mut SqliteConnection,
    user_id: &str,
    list_id: Option<&str>,
) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as(&format!(
        "SELECT {} FROM items i JOIN lists l ON l.id = i.list_id
         WHERE {VISIBLE_LISTS} AND i.deleted_at IS NULL AND (?2 IS NULL OR i.list_id = ?2)
         ORDER BY i.list_id, i.position, i.id",
        cols("i.", ITEM_COLS)
    ))
    .bind(user_id)
    .bind(list_id)
    .fetch_all(conn)
    .await
}

/// An item, tombstoned or not (the caller has checked access).
async fn load_item(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Item> {
    sqlx::query_as(&format!(
        "SELECT {} FROM items WHERE id = ?",
        cols("", ITEM_COLS)
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

async fn load_category(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Category> {
    sqlx::query_as(&format!(
        "SELECT {} FROM categories WHERE id = ?",
        cols("", CAT_COLS)
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

/// The name of a live category on `list_id`, or a 400.
async fn category_on_list(
    conn: &mut SqliteConnection,
    list_id: &str,
    category_id: &str,
) -> ApiResult<String> {
    sqlx::query_scalar(
        "SELECT name FROM categories WHERE id = ? AND list_id = ? AND deleted_at IS NULL",
    )
    .bind(category_id)
    .bind(list_id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| bad("that category isn't on this list"))
}

/// The list's category called `cat_name` (compared like catalogue names), if any.
async fn category_named(
    conn: &mut SqliteConnection,
    list_id: &str,
    cat_name: &str,
) -> sqlx::Result<Option<(String, String)>> {
    let want = normalize_name(cat_name);
    let cats: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM categories WHERE list_id = ? AND deleted_at IS NULL ORDER BY position, id",
    )
    .bind(list_id)
    .fetch_all(conn)
    .await?;
    Ok(cats.into_iter().find(|(_, n)| normalize_name(n) == want))
}

async fn bump_item(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    let rev = next_rev(conn).await?;
    sqlx::query("UPDATE items SET updated_at = ?, rev = ? WHERE id = ?")
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
        Err(bad(format!(
            "note must be at most {MAX_NOTE_CHARS} characters"
        )))
    } else {
        Ok(s.to_string())
    }
}

fn check_step(s: Option<f64>) -> ApiResult<Option<f64>> {
    s.map(amount::check_amount).transpose().map_err(bad)
}

// ---- categories ----------------------------------------------------------------------

#[utoipa::path(get, path = "/lists/{id}/categories", tag = "shopping", summary = "A list's categories, in order", params(("id" = String, Path, description = "List ULID")), responses((status = 200, body = Vec<Category>), (status = 404, body = crate::error::Problem)))]
pub async fn get_categories(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
) -> ApiResult<Json<Vec<Category>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_list(&mut conn, &user, &list_id, Need::Read).await?;
    Ok(Json(list_categories(&mut conn, &list_id).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateCategory {
    /// Optional client-chosen ULID.
    id: Option<String>,
    name: String,
}

#[utoipa::path(post, path = "/lists/{id}/categories", tag = "shopping", summary = "Add a category at the end of a list", params(("id" = String, Path, description = "List ULID")), request_body = CreateCategory, responses((status = 200, body = Category), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, description = "The list already has a category of that name", body = crate::error::Problem)))]
pub async fn create_category(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(c): Json<CreateCategory>,
) -> ApiResult<Json<Category>> {
    let id = id_or_new(c.id)?;
    let cname = name("name", &c.name, MAX_NAME_CHARS)?;
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::EditItems).await?;
    let existing: Option<String> =
        sqlx::query_scalar("SELECT list_id FROM categories WHERE id = ? AND deleted_at IS NULL")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(l) = existing {
        return if l == list_id {
            Ok(Json(load_category(&mut tx, &id).await?))
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    if category_named(&mut tx, &list_id, &cname).await?.is_some() {
        return Err(AppError::Conflict(
            "this list already has a category of that name".into(),
        ));
    }
    let position = next_position(&mut tx, "categories", "list_id", &list_id).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "INSERT INTO categories (id, list_id, name, position, created_at, updated_at, rev) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&list_id)
    .bind(&cname)
    .bind(&position)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *tx)
    .await
    .map_err(insert_err)?;
    let cat = load_category(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(cat))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchCategory {
    name: Option<String>,
}

#[utoipa::path(patch, path = "/categories/{id}", tag = "shopping", summary = "Rename a category", params(("id" = String, Path, description = "ULID")), request_body = PatchCategory, responses((status = 200, body = Category), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem), (status = 409, body = crate::error::Problem)))]
pub async fn patch_category(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchCategory>,
) -> ApiResult<Json<Category>> {
    let mut tx = state.db.write.begin().await?;
    let list_id = visibility::require_category(&mut tx, &user, &id, Need::EditItems).await?;
    if let Some(n) = p.name {
        let n = name("name", &n, MAX_NAME_CHARS)?;
        if category_named(&mut tx, &list_id, &n)
            .await?
            .is_some_and(|(other, _)| other != id)
        {
            return Err(AppError::Conflict(
                "this list already has a category of that name".into(),
            ));
        }
        sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
            .bind(&n)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE categories SET updated_at = ?, rev = ? WHERE id = ?")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let cat = load_category(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(cat))
}

#[utoipa::path(delete, path = "/categories/{id}", tag = "shopping", summary = "Delete a category; its items move to Other", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 404, body = crate::error::Problem)))]
pub async fn delete_category(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_category(&mut tx, &user, &id, Need::EditItems).await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    sqlx::query(
        "UPDATE items SET category_id = NULL, updated_at = ?1, rev = ?2 WHERE category_id = ?3",
    )
    .bind(&ts)
    .bind(rev)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE categories SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
        .bind(&ts)
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- adding items ----------------------------------------------------------------------

/// A new item. Give either `text` (a quick-add line) or `name`.
#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct NewItem {
    /// Optional client-chosen ULID; retrying with it is safe.
    pub id: Option<String>,
    /// A quick-add line such as "flour 1 kg" or "2 milk": name, amount and unit in one.
    pub text: Option<String>,
    pub name: Option<String>,
    /// Without one, your catalogue's usual amount and unit for this name (or 1).
    pub amount: Option<f64>,
    /// Empty or absent for pieces.
    pub unit: Option<String>,
    pub step: Option<f64>,
    pub note: Option<String>,
    /// Absent: the category it usually goes into; null: *Other*.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub category_id: Option<Option<String>>,
    /// A category of the list by name, used when `category_id` is absent.
    pub category_name: Option<String>,
    /// Reference in another system (e.g. a Mealie item id).
    pub external_ref: Option<String>,
}

/// The item added, or the open item the amount was added to.
#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct AddedItem {
    pub item: Item,
    /// Set when the name was already open on the list: that item's amount before this add.
    pub merged_from: Option<f64>,
}

fn source_for(user: &AuthUser) -> &'static str {
    match user.credential {
        Credential::Session(_) => "manual",
        Credential::Token { .. } => "api",
    }
}

#[derive(sqlx::FromRow)]
struct OpenItem {
    id: String,
    name: String,
    amount: f64,
    unit: Option<String>,
    category_id: Option<String>,
}

/// Add an item to a list the caller may edit: fill in the catalogue's usual amount and
/// category, fold it into an open item of the same name (duplicate guard), and record the
/// use in the user's catalogue.
pub async fn add_item(
    conn: &mut SqliteConnection,
    user_id: &str,
    list_id: &str,
    n: NewItem,
    source: &str,
) -> ApiResult<AddedItem> {
    let id = id_or_new(n.id)?;
    // A retry of a create that already happened returns its result.
    let existing: Option<String> = sqlx::query_scalar("SELECT list_id FROM items WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(l) = existing {
        return if l == list_id {
            Ok(AddedItem {
                item: load_item(conn, &id).await?,
                merged_from: None,
            })
        } else {
            Err(AppError::Conflict("id already in use".into()))
        };
    }
    let merged: Option<String> = sqlx::query_scalar("SELECT item_id FROM item_merges WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(target) = merged {
        return Ok(AddedItem {
            item: load_item(conn, &target).await?,
            merged_from: None,
        });
    }

    let parsed = match (&n.text, &n.name) {
        (Some(t), _) => amount::parse(t),
        (None, Some(nm)) => amount::Parsed {
            name: nm.clone(),
            amount: None,
            unit: None,
        },
        (None, None) => return Err(bad("give a name or a text")),
    };
    let iname = name("name", &parsed.name, MAX_NAME_CHARS)?;
    let normalized = normalize_name(&iname);
    let given_unit = match (&n.unit, n.amount) {
        (Some(u), _) => amount::check_unit(u).map_err(bad)?,
        (None, Some(_)) => None,
        (None, None) => parsed.unit.clone(),
    };
    let known = catalog::lookup(conn, user_id, &normalized).await?;
    let step = check_step(n.step)?;
    let (amt, unit, step) = match (n.amount.or(parsed.amount), &known) {
        (Some(a), _) => (amount::check_amount(a).map_err(bad)?, given_unit, step),
        (None, Some(k)) if given_unit.is_none() && n.unit.is_none() => {
            (k.amount, k.unit.clone(), step.or(k.step))
        }
        (None, _) => (1.0, given_unit, step),
    };
    let item_note = note(n.note.as_deref().unwrap_or(""))?;
    let external_ref = n
        .external_ref
        .map(|r| name("external_ref", &r, 200))
        .transpose()?;

    let category = match n.category_id {
        Some(Some(c)) => {
            let cname = category_on_list(conn, list_id, &c).await?;
            Some((c, cname))
        }
        Some(None) => None,
        None => match n
            .category_name
            .or_else(|| known.as_ref().and_then(|k| k.category_name.clone()))
        {
            Some(cn) => category_named(conn, list_id, &cn).await?,
            None => None,
        },
    };
    let shop_id: String = sqlx::query_scalar("SELECT shop_id FROM lists WHERE id = ?")
        .bind(list_id)
        .fetch_one(&mut *conn)
        .await?;

    // Duplicate guard: the same name open on the list gets the amount added.
    let open: Vec<OpenItem> = sqlx::query_as(
        "SELECT id, name, amount, unit, category_id FROM items
         WHERE list_id = ? AND deleted_at IS NULL AND checked_at IS NULL ORDER BY position, id",
    )
    .bind(list_id)
    .fetch_all(&mut *conn)
    .await?;
    let ts = now();
    for OpenItem {
        id: oid,
        name: oname,
        amount: oamount,
        unit: ounit,
        category_id: ocat,
    } in open
    {
        if normalize_name(&oname) != normalized {
            continue;
        }
        let Some(total) = amount::combine(oamount, ounit.as_deref(), amt, unit.as_deref()) else {
            continue;
        };
        let cat_name = match &ocat {
            Some(c) => category_on_list(conn, list_id, c).await.ok(),
            None => None,
        };
        let catalog_id = catalog::record(
            conn,
            user_id,
            &Use {
                name: &iname,
                category_name: cat_name.as_deref(),
                amount: amt,
                unit: unit.as_deref(),
                step,
                shop_id: Some(&shop_id),
            },
        )
        .await?;
        let rev = next_rev(conn).await?;
        sqlx::query(
            "UPDATE items SET amount = ?, catalog_item_id = COALESCE(catalog_item_id, ?), updated_at = ?, rev = ? WHERE id = ?",
        )
        .bind(total)
        .bind(&catalog_id)
        .bind(&ts)
        .bind(rev)
        .bind(&oid)
        .execute(&mut *conn)
        .await?;
        sqlx::query("INSERT INTO item_merges (id, item_id, created_at) VALUES (?,?,?)")
            .bind(&id)
            .bind(&oid)
            .bind(&ts)
            .execute(&mut *conn)
            .await?;
        return Ok(AddedItem {
            item: load_item(conn, &oid).await?,
            merged_from: Some(oamount),
        });
    }

    let catalog_id = catalog::record(
        conn,
        user_id,
        &Use {
            name: &iname,
            category_name: category.as_ref().map(|(_, n)| n.as_str()),
            amount: amt,
            unit: unit.as_deref(),
            step,
            shop_id: Some(&shop_id),
        },
    )
    .await?;
    let position = next_position(conn, "items", "list_id", list_id).await?;
    let rev = next_rev(conn).await?;
    sqlx::query(
        "INSERT INTO items (id, list_id, category_id, catalog_item_id, name, amount, unit, step, note,
                            position, added_by, source, external_ref, created_at, updated_at, rev)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(list_id)
    .bind(category.map(|(c, _)| c))
    .bind(&catalog_id)
    .bind(&iname)
    .bind(amt)
    .bind(&unit)
    .bind(step)
    .bind(&item_note)
    .bind(&position)
    .bind(user_id)
    .bind(source)
    .bind(&external_ref)
    .bind(&ts)
    .bind(&ts)
    .bind(rev)
    .execute(&mut *conn)
    .await
    .map_err(insert_err)?;
    Ok(AddedItem {
        item: load_item(conn, &id).await?,
        merged_from: None,
    })
}

#[utoipa::path(get, path = "/lists/{id}/items", tag = "shopping", summary = "A list's items (open and in the cart)", params(("id" = String, Path, description = "List ULID")), responses((status = 200, body = Vec<Item>), (status = 404, body = crate::error::Problem)))]
pub async fn get_items(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
) -> ApiResult<Json<Vec<Item>>> {
    let mut conn = state.db.read.acquire().await?;
    visibility::require_list(&mut conn, &user, &list_id, Need::Read).await?;
    Ok(Json(
        visible_items(&mut conn, user.id(), Some(&list_id)).await?,
    ))
}

#[utoipa::path(post, path = "/lists/{id}/items", tag = "shopping", summary = "Add an item (an open item of the same name gets the amount added)", params(("id" = String, Path, description = "List ULID")), request_body = NewItem, responses((status = 200, body = AddedItem), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(n): Json<NewItem>,
) -> ApiResult<Json<AddedItem>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::EditItems).await?;
    let added = add_item(&mut tx, user.id(), &list_id, n, source_for(&user)).await?;
    tx.commit().await?;
    Ok(Json(added))
}

#[utoipa::path(post, path = "/shops/{id}/items", tag = "shopping", summary = "Add an item to the shop's default list", params(("id" = String, Path, description = "Shop ULID")), request_body = NewItem, responses((status = 200, body = AddedItem), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_shop_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<String>,
    Json(n): Json<NewItem>,
) -> ApiResult<Json<AddedItem>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_shop(&mut tx, &user, &shop_id).await?;
    let list_id: Option<String> =
        sqlx::query_scalar("SELECT default_list_id FROM shops WHERE id = ?")
            .bind(&shop_id)
            .fetch_one(&mut *tx)
            .await?;
    let list_id = list_id.ok_or(AppError::NotFound)?;
    visibility::require_list(&mut tx, &user, &list_id, Need::EditItems).await?;
    let added = add_item(&mut tx, user.id(), &list_id, n, source_for(&user)).await?;
    tx.commit().await?;
    Ok(Json(added))
}

// ---- changing items --------------------------------------------------------------------

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PatchItem {
    name: Option<String>,
    amount: Option<f64>,
    /// Null or empty for pieces.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    unit: Option<Option<String>>,
    /// Null for the unit's usual step.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<f64>)]
    step: Option<Option<f64>>,
    note: Option<String>,
    /// A category of the same list, or null for *Other*. Remembered for next time.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    category_id: Option<Option<String>>,
    /// `true` puts it in the cart, `false` takes it out.
    checked: Option<bool>,
}

#[utoipa::path(patch, path = "/items/{id}", tag = "shopping", summary = "Edit or tick an item", params(("id" = String, Path, description = "ULID")), request_body = PatchItem, responses((status = 200, body = Item), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn patch_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<PatchItem>,
) -> ApiResult<Json<Item>> {
    let mut tx = state.db.write.begin().await?;
    let list_id = visibility::require_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    let item = load_item(&mut tx, &id).await?;
    if let Some(n) = p.name {
        let n = name("name", &n, MAX_NAME_CHARS)?;
        // Link to the catalogue entry of the new name, creating it if needed.
        let catalog_id = match catalog::lookup(&mut tx, user.id(), &normalize_name(&n)).await? {
            Some(k) => k.id,
            None => {
                let shop_id: String = sqlx::query_scalar("SELECT shop_id FROM lists WHERE id = ?")
                    .bind(&list_id)
                    .fetch_one(&mut *tx)
                    .await?;
                catalog::record(
                    &mut tx,
                    user.id(),
                    &Use {
                        name: &n,
                        category_name: None,
                        amount: item.amount,
                        unit: item.unit.as_deref(),
                        step: item.step,
                        shop_id: Some(&shop_id),
                    },
                )
                .await?
            }
        };
        sqlx::query("UPDATE items SET name = ?, catalog_item_id = ? WHERE id = ?")
            .bind(&n)
            .bind(&catalog_id)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(a) = p.amount {
        sqlx::query("UPDATE items SET amount = ? WHERE id = ?")
            .bind(amount::check_amount(a).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(u) = p.unit {
        sqlx::query("UPDATE items SET unit = ? WHERE id = ?")
            .bind(amount::check_unit(u.as_deref().unwrap_or("")).map_err(bad)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(s) = p.step {
        sqlx::query("UPDATE items SET step = ? WHERE id = ?")
            .bind(check_step(s)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(n) = p.note {
        sqlx::query("UPDATE items SET note = ? WHERE id = ?")
            .bind(note(&n)?)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(c) = p.category_id {
        let cat_name = match &c {
            Some(c) => Some(category_on_list(&mut tx, &list_id, c).await?),
            None => None,
        };
        sqlx::query("UPDATE items SET category_id = ? WHERE id = ?")
            .bind(&c)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        let catalog_id: Option<String> =
            sqlx::query_scalar("SELECT catalog_item_id FROM items WHERE id = ?")
                .bind(&id)
                .fetch_one(&mut *tx)
                .await?;
        if let Some(cid) = catalog_id {
            catalog::learn_category(&mut tx, user.id(), &cid, cat_name.as_deref()).await?;
        }
    }
    if let Some(checked) = p.checked {
        sqlx::query(
            "UPDATE items SET checked_at = CASE WHEN ?1 THEN COALESCE(checked_at, ?2) END,
                              checked_by = CASE WHEN ?1 THEN COALESCE(checked_by, ?3) END
             WHERE id = ?4",
        )
        .bind(checked)
        .bind(now())
        .bind(user.id())
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    }
    bump_item(&mut tx, &id).await?;
    let item = load_item(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(item))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct StepBy {
    /// Presses of + (positive) or − (negative).
    by: i32,
}

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Stepped {
    pub item: Item,
    /// − at the smallest amount removed the item (restore it to undo).
    pub removed: bool,
}

#[utoipa::path(post, path = "/items/{id}/step", tag = "shopping", summary = "Change the amount by its step (−/+)", params(("id" = String, Path, description = "ULID")), request_body = StepBy, responses((status = 200, body = Stepped), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn step_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(s): Json<StepBy>,
) -> ApiResult<Json<Stepped>> {
    if s.by == 0 || s.by.abs() > 1000 {
        return Err(bad("by must be between -1000 and 1000, not 0"));
    }
    let mut tx = state.db.write.begin().await?;
    visibility::require_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    let item = load_item(&mut tx, &id).await?;
    let mut a = Some(item.amount);
    for _ in 0..s.by.abs() {
        a = a.and_then(|a| amount::step(a, item.unit.as_deref(), item.step, s.by > 0));
    }
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    match a {
        Some(a) => {
            sqlx::query("UPDATE items SET amount = ?, updated_at = ?, rev = ? WHERE id = ?")
                .bind(a)
                .bind(&ts)
                .bind(rev)
                .bind(&id)
                .execute(&mut *tx)
                .await?
        }
        None => {
            sqlx::query("UPDATE items SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
                .bind(&ts)
                .bind(rev)
                .bind(&id)
                .execute(&mut *tx)
                .await?
        }
    };
    let item = load_item(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(Stepped {
        item,
        removed: a.is_none(),
    }))
}

#[utoipa::path(delete, path = "/items/{id}", tag = "shopping", summary = "Remove an item (restore it to undo)", params(("id" = String, Path, description = "ULID")), responses((status = 204, description = "Done"), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_item(&mut tx, &user, &id, Need::EditItems, false).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE items SET deleted_at = ?1, updated_at = ?1, rev = ?2 WHERE id = ?3")
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ItemIds {
    ids: Vec<String>,
}

#[utoipa::path(post, path = "/items/restore", tag = "shopping", summary = "Undo: bring removed items back (ones you can't edit are skipped)", request_body = ItemIds, responses((status = 200, body = Vec<Item>), (status = 400, body = crate::error::Problem)))]
pub async fn restore_items(
    State(state): State<AppState>,
    user: AuthUser,
    Json(r): Json<ItemIds>,
) -> ApiResult<Json<Vec<Item>>> {
    if r.ids.len() > 1000 {
        return Err(bad("at most 1000 items at once"));
    }
    let mut tx = state.db.write.begin().await?;
    let rev = next_rev(&mut tx).await?;
    let ts = now();
    let mut out = Vec::new();
    for id in &r.ids {
        if visibility::require_item(&mut tx, &user, id, Need::EditItems, true)
            .await
            .is_err()
        {
            continue;
        }
        // A category deleted meanwhile leaves the item under Other.
        sqlx::query(
            "UPDATE items SET deleted_at = NULL, updated_at = ?1, rev = ?2,
                 category_id = (SELECT c.id FROM categories c WHERE c.id = items.category_id AND c.deleted_at IS NULL)
             WHERE id = ?3",
        )
        .bind(&ts)
        .bind(rev)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        out.push(load_item(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(Json(out))
}

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Finished {
    /// The items taken off the list (restore them to undo).
    pub removed: Vec<String>,
}

#[utoipa::path(post, path = "/lists/{id}/finish", tag = "shopping", summary = "Finish shopping: remove the items in the cart", params(("id" = String, Path, description = "List ULID")), responses((status = 200, body = Finished), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn finish(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
) -> ApiResult<Json<Finished>> {
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::EditItems).await?;
    let rev = next_rev(&mut tx).await?;
    let removed: Vec<String> = sqlx::query_scalar(
        "UPDATE items SET deleted_at = ?1, updated_at = ?1, rev = ?2
         WHERE list_id = ?3 AND deleted_at IS NULL AND checked_at IS NOT NULL RETURNING id",
    )
    .bind(now())
    .bind(rev)
    .bind(&list_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(Finished { removed }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct StapleToAdd {
    staple_id: String,
    /// Optional client-chosen ULID for the new item.
    id: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddStaples {
    staples: Vec<StapleToAdd>,
}

#[utoipa::path(post, path = "/lists/{id}/staples", tag = "shopping", summary = "Add staples of the list's shop to the list", params(("id" = String, Path, description = "List ULID")), request_body = AddStaples, responses((status = 200, body = Vec<AddedItem>), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn add_staples(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(a): Json<AddStaples>,
) -> ApiResult<Json<Vec<AddedItem>>> {
    if a.staples.len() > 500 {
        return Err(bad("at most 500 staples at once"));
    }
    let mut tx = state.db.write.begin().await?;
    visibility::require_list(&mut tx, &user, &list_id, Need::EditItems).await?;
    let list_shop: String = sqlx::query_scalar("SELECT shop_id FROM lists WHERE id = ?")
        .bind(&list_id)
        .fetch_one(&mut *tx)
        .await?;
    let mut out = Vec::with_capacity(a.staples.len());
    for s in a.staples {
        let shop = visibility::require_staple(&mut tx, &user, &s.staple_id).await?;
        if shop != list_shop {
            return Err(bad("that staple belongs to another shop"));
        }
        let (sname, samount, sunit, scat): (String, f64, Option<String>, Option<String>) =
            sqlx::query_as("SELECT name, amount, unit, category_name FROM staples WHERE id = ?")
                .bind(&s.staple_id)
                .fetch_one(&mut *tx)
                .await?;
        let n = NewItem {
            id: s.id,
            name: Some(sname),
            amount: Some(samount),
            unit: Some(sunit.unwrap_or_default()),
            category_name: scat,
            ..Default::default()
        };
        out.push(add_item(&mut tx, user.id(), &list_id, n, "staple").await?);
    }
    tx.commit().await?;
    Ok(Json(out))
}

// ---- moving --------------------------------------------------------------------------

#[utoipa::path(post, path = "/categories/{id}/move", tag = "shopping", summary = "Reorder a category within its list", params(("id" = String, Path, description = "ULID")), request_body = Place, responses((status = 200, body = Category), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_category(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(place): Json<Place>,
) -> ApiResult<Json<Category>> {
    let mut tx = state.db.write.begin().await?;
    let list_id = visibility::require_category(&mut tx, &user, &id, Need::EditItems).await?;
    let scope = Scope {
        table: "categories",
        column: "list_id",
        id: &list_id,
    };
    let key = scope.key_for(&mut tx, &id, &place).await?;
    let rev = next_rev(&mut tx).await?;
    sqlx::query("UPDATE categories SET position = ?, updated_at = ?, rev = ? WHERE id = ?")
        .bind(key)
        .bind(now())
        .bind(rev)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let cat = load_category(&mut tx, &id).await?;
    tx.commit().await?;
    Ok(Json(cat))
}

/// Move one item: to another place among the target list's items, another category, and
/// maybe another list (where it keeps a category of the same name, if there is one).
async fn move_one(
    conn: &mut SqliteConnection,
    user: &AuthUser,
    id: &str,
    target: Option<&str>,
    category_id: Option<Option<String>>,
    place: &Place,
) -> ApiResult<Item> {
    let from = visibility::require_item(conn, user, id, Need::EditItems, false).await?;
    let target = target.unwrap_or(&from).to_string();
    if target != from {
        visibility::require_list(conn, user, &target, Need::EditItems).await?;
    }
    let item = load_item(conn, id).await?;
    let category = match category_id {
        Some(Some(c)) => {
            let cname = category_on_list(conn, &target, &c).await?;
            // Dropping it into a category is remembered, like choosing one in the editor.
            if let Some(cid) = item
                .catalog_item_id
                .as_deref()
                .filter(|_| item.category_id.as_deref() != Some(&c))
            {
                catalog::learn_category(conn, user.id(), cid, Some(&cname)).await?;
            }
            Some(c)
        }
        Some(None) => None,
        None if target == from => item.category_id.clone(),
        None => match &item.category_id {
            Some(c) => match category_on_list(conn, &from, c).await.ok() {
                Some(cname) => category_named(conn, &target, &cname)
                    .await?
                    .map(|(id, _)| id),
                None => None,
            },
            None => None,
        },
    };
    let scope = Scope {
        table: "items",
        column: "list_id",
        id: &target,
    };
    let key = scope.key_for(conn, id, place).await?;
    let rev = next_rev(conn).await?;
    sqlx::query(
        "UPDATE items SET list_id = ?, category_id = ?, position = ?, updated_at = ?, rev = ? WHERE id = ?",
    )
    .bind(&target)
    .bind(&category)
    .bind(key)
    .bind(now())
    .bind(rev)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(load_item(conn, id).await?)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MoveItem {
    /// Put it right after this item (of the target list).
    after: Option<String>,
    /// Or right before this one. Neither: at the end.
    before: Option<String>,
    /// Move it to another list you can edit.
    list_id: Option<String>,
    /// A category of the target list, or null for *Other*. Absent: unchanged, or on another
    /// list the category of the same name.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    category_id: Option<Option<String>>,
}

#[utoipa::path(post, path = "/items/{id}/move", tag = "shopping", summary = "Reorder an item, move it to another category or another list", params(("id" = String, Path, description = "ULID")), request_body = MoveItem, responses((status = 200, body = Item), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(m): Json<MoveItem>,
) -> ApiResult<Json<Item>> {
    let mut tx = state.db.write.begin().await?;
    let place = Place {
        after: m.after,
        before: m.before,
    };
    let item = move_one(
        &mut tx,
        &user,
        &id,
        m.list_id.as_deref(),
        m.category_id,
        &place,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(item))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MoveItems {
    ids: Vec<String>,
    /// The list to move them to (the end of it, in the order given).
    list_id: String,
    /// A category of that list, or null for *Other*. Absent: each keeps a category of the
    /// same name, if the list has one.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    category_id: Option<Option<String>>,
}

#[utoipa::path(post, path = "/items/move", tag = "shopping", summary = "Move several items to another list", request_body = MoveItems, responses((status = 200, body = Vec<Item>), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn move_items(
    State(state): State<AppState>,
    user: AuthUser,
    Json(m): Json<MoveItems>,
) -> ApiResult<Json<Vec<Item>>> {
    if m.ids.is_empty() || m.ids.len() > 500 {
        return Err(bad("move 1 to 500 items at once"));
    }
    let mut tx = state.db.write.begin().await?;
    let mut out = Vec::with_capacity(m.ids.len());
    for id in &m.ids {
        out.push(
            move_one(
                &mut tx,
                &user,
                id,
                Some(&m.list_id),
                m.category_id.clone(),
                &Place::default(),
            )
            .await?,
        );
    }
    tx.commit().await?;
    Ok(Json(out))
}
