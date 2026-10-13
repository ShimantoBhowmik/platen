//! Runtime configuration, read from environment variables (and `.env` in development).

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, bail};
use secrecy::{ExposeSecret, SecretString};

/// Shortest accepted `TOKEN_SECRET`, in bytes. HMAC-SHA256 gains nothing past 64.
const MIN_SECRET_BYTES: usize = 32;
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";
const DEFAULT_MAX_DB_CONNECTIONS: u32 = 16;

/// Validated server settings.
///
/// Secrets are wrapped in `SecretString`, whose `Debug` prints a placeholder instead of the
/// value, so a logged config never leaks them.
#[derive(Debug)]
pub struct Config {
    /// `DATABASE_URL`: Postgres connection string. Required.
    pub database_url: SecretString,
    /// `TOKEN_SECRET`: key that signs test tokens, at least 32 bytes. Required.
    pub token_secret: SecretString,
    /// `BIND_ADDR`: socket to listen on. Defaults to `0.0.0.0:3000`.
    pub bind_addr: SocketAddr,
    /// `STATIC_DIR`: built frontend to serve beside the API (`dx bundle` output). Unset
    /// serves the API only.
    pub static_dir: Option<PathBuf>,
    /// `MAX_DB_CONNECTIONS`: size of the Postgres pool. Defaults to 16.
    pub max_db_connections: u32,
}

impl Config {
    /// Reads and validates the configuration from the environment.
    ///
    /// # Errors
    ///
    /// Fails if a required variable is missing or any variable is malformed.
    pub fn from_env() -> anyhow::Result<Self> {
        let token_secret = SecretString::from(required("TOKEN_SECRET")?);
        if token_secret.expose_secret().len() < MIN_SECRET_BYTES {
            bail!("TOKEN_SECRET must be at least {MIN_SECRET_BYTES} bytes");
        }
        Ok(Self {
            database_url: SecretString::from(required("DATABASE_URL")?),
            token_secret,
            bind_addr: optional("BIND_ADDR")
                .as_deref()
                .unwrap_or(DEFAULT_BIND_ADDR)
                .parse()
                .context("BIND_ADDR must be a socket address such as 0.0.0.0:3000")?,
            static_dir: optional("STATIC_DIR").map(PathBuf::from),
            max_db_connections: optional("MAX_DB_CONNECTIONS")
                .map(|count| count.parse())
                .transpose()
                .context("MAX_DB_CONNECTIONS must be a positive integer")?
                .unwrap_or(DEFAULT_MAX_DB_CONNECTIONS),
        })
    }
}

fn required(key: &str) -> anyhow::Result<String> {
    optional(key).with_context(|| format!("{key} must be set"))
}

/// Reads a variable, treating unset and empty the same.
fn optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}
