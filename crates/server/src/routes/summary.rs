//! The summary for other apps (SPEC §6.11): what needs buying on each list you can see, with
//! a link back. Streamline polls it with a read token to keep one "Go shopping" task per list.

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use shoppingcart_domain::amount;
use ts_rs::TS;

use crate::{
    AppState,
    auth::{AuthUser, is_secure},
    error::ApiResult,
    visibility::{self, VISIBLE_LISTS},
};

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct SummaryItem {
    pub id: String,
    pub name: String,
    /// "2", "1.5 kg"; none for a single piece.
    pub amount: Option<String>,
    pub category: Option<String>,
}

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct ListSummary {
    pub list_id: String,
    pub list_name: String,
    pub shop_id: String,
    pub shop_name: String,
    pub owner_name: String,
    #[ts(type = "'owner' | 'editor' | 'viewer'")]
    pub role: String,
    /// Items not yet in the cart.
    #[ts(type = "number")]
    pub open_count: i64,
    /// The first open items, in list order.
    pub items: Vec<SummaryItem>,
    /// When anything on the list last changed (also items removed).
    pub updated_at: String,
    /// Opens the list in ShoppingCart (from `PUBLIC_URL`, else the address used to call this).
    pub url: String,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SummaryQuery {
    /// Items per list (default 20, at most 200).
    limit: Option<usize>,
    /// Only the lists of this shop.
    shop_id: Option<String>,
    /// Only this list.
    list_id: Option<String>,
    /// Leave out lists with nothing to buy.
    #[serde(default)]
    non_empty: bool,
}

/// The address people use: `PUBLIC_URL`, else what this request came to (the proxy's
/// forwarded host and scheme only when `TRUST_PROXY` is set).
pub fn base_url(state: &AppState, headers: &HeaderMap) -> String {
    if let Some(u) = &state.config.public_url {
        return u.clone();
    }
    let header = |n: &str| {
        headers
            .get(n)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').next().unwrap_or("").trim().to_string())
    };
    let host = state
        .config
        .trust_proxy
        .then(|| header("x-forwarded-host"))
        .flatten()
        .or_else(|| header("host"))
        .unwrap_or_else(|| "localhost".into());
    let scheme = if is_secure(state, headers) {
        "https"
    } else {
        "http"
    };
    format!("{scheme}://{host}")
}

/// id, name, amount, unit, category name.
type ItemRow = (String, String, f64, Option<String>, Option<String>);

#[utoipa::path(get, path = "/summary", tag = "integrations", summary = "What needs buying on each list you can see, with links (for Streamline and dashboards; a read token is enough)", params(SummaryQuery), responses((status = 200, body = Vec<ListSummary>), (status = 401, body = crate::error::Problem)))]
pub async fn summary(
    State(state): State<AppState>,
    user: AuthUser,
    headers: HeaderMap,
    Query(q): Query<SummaryQuery>,
) -> ApiResult<Json<Vec<ListSummary>>> {
    let limit = q.limit.unwrap_or(20).min(200);
    let base = base_url(&state, &headers);
    let mut conn = state.db.read.acquire().await?;
    let role = visibility::role_sql();
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        name: String,
        shop_id: String,
        shop_name: String,
        owner_name: String,
        role: String,
        open_count: i64,
        updated_at: String,
    }
    let lists: Vec<Row> = sqlx::query_as(&format!(
        "SELECT l.id, l.name, l.shop_id, s.name AS shop_name, u.display_name AS owner_name, {role} AS role,
                (SELECT COUNT(*) FROM items i WHERE i.list_id = l.id AND i.deleted_at IS NULL AND i.checked_at IS NULL) AS open_count,
                MAX(l.updated_at, COALESCE((SELECT MAX(i.updated_at) FROM items i WHERE i.list_id = l.id), '')) AS updated_at
         FROM lists l JOIN shops s ON s.id = l.shop_id JOIN users u ON u.id = l.owner_id
         WHERE {VISIBLE_LISTS} AND l.kind = 'list' AND l.archived_at IS NULL
           AND (?2 IS NULL OR l.shop_id = ?2) AND (?3 IS NULL OR l.id = ?3)
         ORDER BY s.position, l.position, l.id"
    ))
    .bind(user.id())
    .bind(&q.shop_id)
    .bind(&q.list_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(lists.len());
    for l in lists {
        if q.non_empty && l.open_count == 0 {
            continue;
        }
        let rows: Vec<ItemRow> = sqlx::query_as(
            "SELECT i.id, i.name, i.amount, i.unit, c.name FROM items i
             LEFT JOIN categories c ON c.id = i.category_id AND c.deleted_at IS NULL
             WHERE i.list_id = ? AND i.deleted_at IS NULL AND i.checked_at IS NULL
             ORDER BY c.position IS NULL, c.position, i.position, i.id LIMIT ?",
        )
        .bind(&l.id)
        .bind(limit as i64)
        .fetch_all(&mut *conn)
        .await?;
        out.push(ListSummary {
            url: format!("{base}/list/{}", l.id),
            list_id: l.id,
            list_name: l.name,
            shop_id: l.shop_id,
            shop_name: l.shop_name,
            owner_name: l.owner_name,
            role: l.role,
            open_count: l.open_count,
            items: rows
                .into_iter()
                .map(|(id, name, a, unit, category)| SummaryItem {
                    id,
                    name,
                    amount: (a != 1.0 || unit.is_some())
                        .then(|| amount::format(a, unit.as_deref())),
                    category,
                })
                .collect(),
            updated_at: l.updated_at,
        });
    }
    Ok(Json(out))
}
