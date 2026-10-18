//! Backups (SPEC §6.14): a consistent copy of the database (`VACUUM INTO`, safe while the app
//! runs) in `data/backups/`, made nightly and on demand, keeping the last `BACKUP_KEEP`.
//! Pictures are in `data/images/` and the key for stored tokens in `data/secret.key`: back up
//! the whole `data` folder for those.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::AppState;

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// Names we make, and the only ones we serve: `shoppingcart-YYYYMMDD-HHMMSS.db`.
pub fn valid_name(name: &str) -> bool {
    let Some(stamp) = name
        .strip_prefix("shoppingcart-")
        .and_then(|n| n.strip_suffix(".db"))
    else {
        return false;
    };
    stamp.len() == 15
        && stamp
            .chars()
            .enumerate()
            .all(|(i, c)| if i == 8 { c == '-' } else { c.is_ascii_digit() })
}

#[derive(Serialize, TS, utoipa::ToSchema)]
#[ts(export)]
pub struct Backup {
    pub name: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub created_at: String,
}

/// Our backups, newest first.
pub fn list(data_dir: &Path) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(dir(data_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !valid_name(&name) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let created: chrono::DateTime<chrono::Utc> = meta.modified().ok()?.into();
            Some(Backup {
                name,
                bytes: meta.len(),
                created_at: created.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            })
        })
        .collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

/// Make a backup now; returns its name. Older ones beyond `keep` are deleted (at least one
/// is always kept).
pub async fn make(state: &AppState) -> anyhow::Result<String> {
    let d = dir(&state.config.data_dir);
    tokio::fs::create_dir_all(&d).await?;
    let name = format!(
        "shoppingcart-{}.db",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = d.join(&name);
    if path.exists() {
        return Ok(name);
    }
    sqlx::query("VACUUM INTO ?")
        .bind(path.to_string_lossy().to_string())
        .execute(&state.db.write)
        .await?;
    for old in list(&state.config.data_dir)
        .into_iter()
        .skip(state.config.backup_keep.max(1))
    {
        let _ = tokio::fs::remove_file(d.join(old.name)).await;
    }
    Ok(name)
}

/// Nightly: once the newest backup is about a day old (checked hourly).
pub fn spawn_job(state: AppState) {
    if state.config.backup_keep == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let due = list(&state.config.data_dir).first().is_none_or(|b| {
                chrono::DateTime::parse_from_rfc3339(&b.created_at)
                    .map(|t| chrono::Utc::now() - t.to_utc() > chrono::Duration::hours(23))
                    .unwrap_or(true)
            });
            if due {
                match make(&state).await {
                    Ok(n) => tracing::info!("backup {n} made"),
                    Err(e) => tracing::warn!("backup failed: {e}"),
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("shoppingcart-20261018-030000.db"));
        assert!(!valid_name("../shoppingcart.db"));
        assert!(!valid_name("shoppingcart-20261018-030000.db/../../x"));
        assert!(!valid_name("shoppingcart-2026101-0300000.db"));
        assert!(!valid_name("secret.key"));
    }
}
