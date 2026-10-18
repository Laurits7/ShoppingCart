//! Integrations (SPEC §5.5, §6.10): per user, with the token encrypted at rest, run by a
//! background job every few minutes (with back-off after failures) or on "Sync now". A failing
//! integration never affects anything else; its last error is shown in Settings.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use shoppingcart_domain::{catalog::normalize_name, mealie::map};
use sqlx::SqliteConnection;
use ts_rs::TS;

use crate::{
    AppState,
    db::{next_position, next_rev},
    events,
    mealie::{self, Mealie},
    routes::items::{NewItem, add_item},
    util::{new_id, now},
    visibility::{self, Need},
};

/// How often an integration runs when all is well.
pub const EVERY_MINUTES: i64 = 5;
/// Longest wait between tries after failures.
const MAX_BACKOFF_MINUTES: i64 = 6 * 60;

fn yes() -> bool {
    true
}
fn check() -> String {
    "check".into()
}

/// Settings of a Mealie integration (the token is stored separately, encrypted).
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct MealieSettings {
    /// Mealie's address, e.g. `http://mealie:9000`.
    pub base_url: String,
    /// The Mealie shopping list to watch.
    #[serde(default)]
    pub list_id: Option<String>,
    #[serde(default)]
    pub list_name: Option<String>,
    /// Items go to this shop's default list…
    #[serde(default)]
    pub target_shop_id: Option<String>,
    /// …or to this list.
    #[serde(default)]
    pub target_list_id: Option<String>,
    /// A Mealie label puts the item into the category of the same name.
    #[serde(default = "yes")]
    pub labels_as_categories: bool,
    /// Create that category when the list doesn't have it.
    #[serde(default)]
    pub create_categories: bool,
    /// After importing: `check` (tick in Mealie, D-16), `delete`, or `keep`.
    #[serde(default = "check")]
    #[ts(type = "'check' | 'delete' | 'keep'")]
    pub after_import: String,
}

/// An integration as its owner sees it: never the token.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Integration {
    pub id: String,
    #[ts(type = "'mealie'")]
    pub provider: String,
    pub settings: MealieSettings,
    /// Whether a token is stored.
    pub has_secret: bool,
    pub enabled: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
pub struct Row {
    pub id: String,
    pub owner_id: String,
    pub provider: String,
    pub settings: String,
    pub secret: Option<String>,
    pub enabled: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Row {
    pub fn settings(&self) -> MealieSettings {
        serde_json::from_str(&self.settings).unwrap_or_default()
    }

    pub fn view(&self) -> Integration {
        Integration {
            id: self.id.clone(),
            provider: self.provider.clone(),
            settings: self.settings(),
            has_secret: self.secret.is_some(),
            enabled: self.enabled,
            last_sync_at: self.last_sync_at.clone(),
            last_error: self.last_error.clone(),
            last_error_at: self.last_error_at.clone(),
            next_run_at: self.next_run_at.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

pub const COLS: &str = "id, owner_id, provider, settings, secret, enabled, last_sync_at, last_error, last_error_at, next_run_at, created_at, updated_at";

pub async fn load(conn: &mut SqliteConnection, id: &str) -> sqlx::Result<Option<Row>> {
    sqlx::query_as(&format!(
        "SELECT {COLS} FROM integrations WHERE id = ? AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

fn later(minutes: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::minutes(minutes))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// One run at a time (the job and "Sync now" never import the same items twice in parallel).
static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What a run did: how many items it imported, and what went wrong (possibly after importing).
pub struct Outcome {
    pub imported: i64,
    pub error: Option<String>,
}

/// Run an integration now and record the outcome.
pub async fn run(state: &AppState, id: &str) -> anyhow::Result<Outcome> {
    let _one = RUNNING.lock().await;
    let mut conn = state.db.read.acquire().await?;
    let Some(row) = load(&mut conn, id).await? else {
        return Ok(Outcome {
            imported: 0,
            error: Some("no such integration".into()),
        });
    };
    drop(conn);
    let mut imported = 0;
    let result = run_mealie(state, &row, &mut imported).await;
    let ts = now();
    match &result {
        Ok(()) => {
            sqlx::query(
                "UPDATE integrations SET last_sync_at = ?1, last_error = NULL, last_error_at = NULL, failures = 0,
                                         next_run_at = ?2 WHERE id = ?3",
            )
            .bind(&ts)
            .bind(later(EVERY_MINUTES))
            .bind(id)
            .execute(&state.db.write)
            .await?;
        }
        Err(e) => {
            let failures: i64 =
                sqlx::query_scalar("SELECT failures FROM integrations WHERE id = ?")
                    .bind(id)
                    .fetch_one(&state.db.write)
                    .await?;
            let wait = (EVERY_MINUTES << failures.min(8)).min(MAX_BACKOFF_MINUTES);
            sqlx::query(
                "UPDATE integrations SET last_error = ?1, last_error_at = ?2, failures = failures + 1, next_run_at = ?3
                 WHERE id = ?4",
            )
            .bind(e)
            .bind(&ts)
            .bind(later(wait))
            .bind(id)
            .execute(&state.db.write)
            .await?;
        }
    }
    Ok(Outcome {
        imported,
        error: result.err(),
    })
}

/// The list Mealie items go to, if the owner may still add to it.
async fn target_list(
    conn: &mut SqliteConnection,
    owner: &str,
    s: &MealieSettings,
) -> Result<String, String> {
    let list = match (&s.target_list_id, &s.target_shop_id) {
        (Some(l), _) => Some(l.clone()),
        (None, Some(shop)) => sqlx::query_scalar::<_, Option<String>>(
            "SELECT default_list_id FROM shops WHERE id = ? AND owner_id = ? AND deleted_at IS NULL",
        )
        .bind(shop)
        .bind(owner)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?
        .flatten(),
        (None, None) => return Err("choose where Mealie's items should go".into()),
    };
    let list = list.ok_or("the shop or list Mealie's items go to is gone")?;
    let role = visibility::list_role(conn, owner, &list)
        .await
        .map_err(|e| e.to_string())?;
    match role {
        Some(r) if shoppingcart_domain::visibility::allows(r, Need::EditItems) => Ok(list),
        _ => Err("you can no longer add to the list Mealie's items go to".into()),
    }
}

/// The list's category called `name`, created if allowed and missing.
async fn category(
    conn: &mut SqliteConnection,
    list: &str,
    name: &str,
    create: bool,
) -> sqlx::Result<Option<String>> {
    let want = normalize_name(name);
    let cats: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM categories WHERE list_id = ? AND deleted_at IS NULL")
            .bind(list)
            .fetch_all(&mut *conn)
            .await?;
    if let Some((id, _)) = cats.iter().find(|(_, n)| normalize_name(n) == want) {
        return Ok(Some(id.clone()));
    }
    if !create || name.trim().is_empty() {
        return Ok(None);
    }
    let id = new_id();
    let position = next_position(conn, "categories", "list_id", list).await?;
    let rev = next_rev(conn).await?;
    let ts = now();
    sqlx::query("INSERT INTO categories (id, list_id, name, position, created_at, updated_at, rev) VALUES (?,?,?,?,?,?,?)")
        .bind(&id)
        .bind(list)
        .bind(name.trim().chars().take(200).collect::<String>())
        .bind(&position)
        .bind(&ts)
        .bind(&ts)
        .bind(rev)
        .execute(conn)
        .await?;
    Ok(Some(id))
}

/// Import from Mealie, counting into `imported` (it stays counted if a later step fails).
async fn run_mealie(state: &AppState, row: &Row, imported: &mut i64) -> Result<(), String> {
    let s = row.settings();
    let token = row
        .secret
        .as_deref()
        .and_then(|t| state.secrets.open(t))
        .ok_or("the stored Mealie token can't be read (was SECRET_KEY changed?); enter it again")?;
    let list_id = s
        .list_id
        .clone()
        .ok_or("choose which Mealie list to watch")?;
    let target = {
        let mut conn = state.db.read.acquire().await.map_err(|e| e.to_string())?;
        target_list(&mut conn, &row.owner_id, &s).await?
    };
    let m = Mealie {
        http: &state.integration_http,
        base: &s.base_url,
        token: &token,
    };
    let items = m.unchecked(&list_id).await?;

    // Import what's new, in one transaction; links make sure nothing comes twice.
    let from = events::current_rev(&state.db.write)
        .await
        .map_err(|e| e.to_string())?;
    let mut count = 0;
    {
        let mut tx = state.db.write.begin().await.map_err(|e| e.to_string())?;
        for v in &items {
            let Some(ext) = v.get("id").and_then(|i| i.as_str()) else {
                continue;
            };
            let seen: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM external_links WHERE integration_id = ? AND external_id = ?",
            )
            .bind(&row.id)
            .bind(ext)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            if seen.is_some() {
                continue;
            }
            let Some(mapped) = map(&mealie::parts(v)) else {
                continue;
            };
            let category_id = match (&mapped.category, s.labels_as_categories) {
                (Some(c), true) => category(&mut tx, &target, c, s.create_categories)
                    .await
                    .map_err(|e| e.to_string())?,
                _ => None,
            };
            let n = NewItem {
                name: Some(mapped.name.chars().take(200).collect()),
                amount: mapped.amount,
                unit: mapped
                    .unit
                    .clone()
                    .or_else(|| mapped.amount.map(|_| String::new())),
                note: Some(mapped.note.chars().take(1000).collect()),
                category_id: category_id.map(Some),
                external_ref: Some(ext.chars().take(200).collect()),
                ..Default::default()
            };
            let added = add_item(&mut tx, &row.owner_id, &target, n, "mealie")
                .await
                .map_err(|e| format!("couldn't add an item from Mealie: {e:?}"))?;
            sqlx::query("INSERT INTO external_links (integration_id, external_id, item_id, created_at) VALUES (?,?,?,?)")
                .bind(&row.id)
                .bind(ext)
                .bind(&added.item.id)
                .bind(now())
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            count += 1;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
    }
    *imported = count;
    // Live update for everyone who sees the list (this write didn't come through a request).
    if let Ok(to) = events::current_rev(&state.db.write).await
        && to > from
        && let Ok(c) = events::changed(&state.db.write, from, to).await
    {
        state.bus.publish(c);
    }

    // Then empty Mealie's list (all unchecked items there are imported now, including any a
    // failed earlier tick left behind).
    match s.after_import.as_str() {
        "keep" => {}
        "delete" => {
            let ids: Vec<String> = items
                .iter()
                .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(String::from))
                .collect();
            m.delete_items(&ids)
                .await
                .map_err(|e| format!("couldn't remove them from Mealie: {e}"))?;
        }
        _ => m
            .check_items(&items)
            .await
            .map_err(|e| format!("couldn't tick them in Mealie: {e}"))?,
    }
    Ok(())
}

/// The background job: every half minute, run the integrations that are due.
pub fn spawn_job(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            let due: Vec<String> = match sqlx::query_scalar(
                "SELECT id FROM integrations WHERE deleted_at IS NULL AND enabled = 1
                   AND (next_run_at IS NULL OR next_run_at <= ?) ORDER BY next_run_at",
            )
            .bind(now())
            .fetch_all(&state.db.read)
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("integration job: {e}");
                    continue;
                }
            };
            for id in due {
                match run(&state, &id).await {
                    Ok(o) => {
                        if o.imported > 0 {
                            tracing::info!("integration {id}: imported {}", o.imported);
                        }
                        if let Some(e) = o.error {
                            tracing::info!("integration {id}: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("integration {id}: {e}"),
                }
            }
        }
    });
}
