//! Typing-test backend: issues tests, verifies keystroke logs, and keeps leaderboards.

mod api;
mod config;
mod error;
mod mode;
mod scoring;
mod store;
mod token;
mod words;

use anyhow::Context;
use secrecy::ExposeSecret;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::api::AppState;
use crate::config::Config;
use crate::store::PgStore;
use crate::token::TokenSigner;

/// The SQL migrations in `migrations/`, embedded in the binary at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A missing `.env` is normal in production, where variables come from the environment.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(config.database_url.expose_secret())
        .await
        .context("connecting to Postgres")?;
    MIGRATOR.run(&pool).await.context("running migrations")?;

    let state = AppState::new(
        PgStore::new(pool),
        TokenSigner::new(config.token_secret.expose_secret().as_bytes()),
    );
    let app = api::router(state, config.cors_origin);
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("binding {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP")
}

/// Resolves on Ctrl-C or, on Unix, SIGTERM, so in-flight requests finish before exit.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for Ctrl-C");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(%error, "failed to listen for SIGTERM"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}
