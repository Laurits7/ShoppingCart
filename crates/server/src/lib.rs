//! ShoppingCart server: HTTP API, auth and the embedded web app, in one binary.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod positions;
pub mod routes;
pub mod static_files;
pub mod util;
pub mod visibility;

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};

use tokio::{net::TcpListener, sync::Mutex};
use tracing_subscriber::EnvFilter;

pub struct App {
    pub config: config::Config,
    pub db: db::Db,
    /// Sign-in attempts per key (username or IP): (count, window start).
    pub login_failures: Mutex<HashMap<String, (u32, Instant)>>,
}

pub type AppState = Arc<App>;

/// Open the database, run migrations and build the router.
pub async fn build(config: config::Config) -> anyhow::Result<(axum::Router, AppState)> {
    check_data_dir(&config.data_dir)?;
    let db = db::Db::open(&config.data_dir.join("shoppingcart.db")).await?;
    let state = Arc::new(App {
        config,
        db,
        login_failures: Mutex::new(HashMap::new()),
    });
    auth::bootstrap_admin(&state).await?;
    Ok((routes::router(state.clone()), state))
}

/// Fail early, with the fix, when the data folder can't be written (usually its owner
/// differs from the user the container runs as).
fn check_data_dir(dir: &std::path::Path) -> anyhow::Result<()> {
    let probe = dir.join(".write-test");
    std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::write(&probe, b""))
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|e| {
            anyhow::anyhow!(
                "can't write to the data folder {} ({e}). With Docker, make the folder belong to the \
                 user the container runs as, e.g. `sudo chown -R $(id -u):$(id -g) data`, or set \
                 PUID/PGID in docker-compose.yml",
                dir.display()
            )
        })
}

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_env_filter(
            EnvFilter::try_from_env("LOG_LEVEL")
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();
    let config = config::Config::from_env()?;
    let addr = SocketAddr::new(config.bind, config.port);
    let (app, state) = build(config).await?;
    spawn_cleanup(state);
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("ShoppingCart listening on http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Hourly housekeeping: drop expired sessions, tombstones older than 30 days and old
/// duplicate-guard records.
fn spawn_cleanup(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
                .bind(util::now())
                .execute(&state.db.write)
                .await
            {
                tracing::warn!("session cleanup failed: {e}");
            }
            if let Err(e) = db::purge(&state.db.write).await {
                tracing::warn!("tombstone cleanup failed: {e}");
            }
        }
    });
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutting down");
}
