//! Result persistence.
//!
//! [`ResultStore`] is the seam between the HTTP layer and the database, so handlers can be
//! tested against an in-memory store. [`PgStore`] is the production implementation.

use std::future::Future;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::types::Json;
use thiserror::Error;
use uuid::Uuid;

use crate::mode::TestMode;
use crate::scoring::{Breakdown, TestStats, TimelinePoint};

/// Longest allowed display name, in characters.
pub const MAX_NAME_CHARS: usize = 24;

/// A display name for the leaderboard: trimmed, 1 to [`MAX_NAME_CHARS`] characters of
/// letters, digits, spaces and `-_.'`, with at least one letter or digit.
///
/// The allow-list rules out invisible and bidirectional-override characters, which could
/// otherwise make a blank name or visually spoof another name on the board.
///
/// A newtype like this wraps a `String` so the compiler only lets validated names reach the
/// store, the way a Python class with a validating constructor would, but at zero runtime cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct DisplayName(String);

/// Returned when a display name breaks the rules of [`DisplayName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error(
    "name must be 1 to {MAX_NAME_CHARS} letters, digits, spaces or -_.' with at least one \
     letter or digit"
)]
pub struct InvalidName;

impl DisplayName {
    /// Validates and trims a raw name.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidName`] if the trimmed name is too long, has a character outside the
    /// allow-list, or has no letter or digit.
    pub fn parse(raw: &str) -> Result<Self, InvalidName> {
        let name = raw.trim();
        let allowed = |character: char| {
            character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.' | '\'')
        };
        if name.chars().count() > MAX_NAME_CHARS
            || !name.chars().all(allowed)
            || !name.chars().any(char::is_alphanumeric)
        {
            return Err(InvalidName);
        }
        Ok(Self(name.to_owned()))
    }

    /// The name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A verified, stored test result.
#[derive(Debug, Clone, PartialEq)]
pub struct TestResult {
    /// The test id from the token. Unique, so each token is scored once.
    pub id: Uuid,
    /// The test configuration.
    pub mode: TestMode,
    /// Set when the typist entered the result on the leaderboard.
    pub name: Option<DisplayName>,
    /// The verified scores.
    pub stats: TestStats,
    /// Per-key and per-word mistakes, for the learner's feedback.
    pub breakdown: Breakdown,
    /// Speed and errors per second, for the result's chart.
    pub timeline: Vec<TimelinePoint>,
    /// When the server accepted the result.
    pub created_at: DateTime<Utc>,
}

/// One row of a leaderboard: a name's best result in a mode.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LeaderboardEntry {
    /// The typist's display name.
    pub name: DisplayName,
    /// WPM of the name's best result.
    pub wpm: f64,
    /// Accuracy of that result.
    pub accuracy: f64,
    /// Id of that result, for `GET /api/results/{id}`.
    pub result_id: Uuid,
    /// When that result was accepted. Earlier wins a WPM tie.
    pub created_at: DateTime<Utc>,
}

/// Errors from a [`ResultStore`].
#[derive(Debug, Error)]
pub enum StoreError {
    /// A result for this test id already exists.
    #[error("a result for this test was already submitted")]
    Duplicate,
    /// The database failed or returned data that breaks the schema.
    #[error(transparent)]
    Backend(#[from] sqlx::Error),
}

/// Storage for results and leaderboards.
///
/// The methods return `impl Future + Send` instead of using `async fn` so the futures can be
/// awaited inside axum handlers, which run on any thread of the multi-threaded runtime.
pub trait ResultStore: Clone + Send + Sync + 'static {
    /// Stores a result. When it has a name and beats that name's best in its mode, it also
    /// becomes the name's leaderboard entry.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Duplicate`] if the test id was stored before.
    fn save(&self, result: &TestResult) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Looks up a result by test id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Backend`] if the lookup fails.
    fn find(&self, id: Uuid)
    -> impl Future<Output = Result<Option<TestResult>, StoreError>> + Send;

    /// The best result per name in a mode, fastest first, oldest first on ties.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Backend`] if the query fails.
    fn leaderboard(
        &self,
        mode: TestMode,
        limit: u16,
    ) -> impl Future<Output = Result<Vec<LeaderboardEntry>, StoreError>> + Send;
}

/// Postgres-backed [`ResultStore`]. Cloning shares the same connection pool.
#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    /// Wraps a connection pool. Run [`crate::MIGRATOR`] on it first.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// A `results` row as Postgres returns it. Converted to [`TestResult`] right away.
#[derive(sqlx::FromRow)]
struct ResultRow {
    id: Uuid,
    mode: i16,
    name: Option<String>,
    wpm: f64,
    raw_wpm: f64,
    accuracy: f64,
    consistency: f64,
    correct_keystrokes: i32,
    incorrect_keystrokes: i32,
    elapsed_ms: i32,
    breakdown: Json<Breakdown>,
    timeline: Json<Vec<TimelinePoint>>,
    created_at: DateTime<Utc>,
}

/// Converts a column value, reporting schema drift as a decode error instead of panicking.
fn decode<T, S: TryInto<T>>(column: &'static str, value: S) -> Result<T, sqlx::Error> {
    value.try_into().map_err(|_| sqlx::Error::ColumnDecode {
        index: column.to_owned(),
        source: format!("value out of range for {column}").into(),
    })
}

impl TryFrom<ResultRow> for TestResult {
    type Error = sqlx::Error;

    fn try_from(row: ResultRow) -> Result<Self, Self::Error> {
        let mode_code: u8 = decode("mode", row.mode)?;
        Ok(Self {
            id: row.id,
            mode: TestMode::from_code(mode_code).ok_or_else(|| sqlx::Error::ColumnDecode {
                index: "mode".to_owned(),
                source: format!("unknown mode code {mode_code}").into(),
            })?,
            name: row.name.map(DisplayName),
            stats: TestStats {
                wpm: row.wpm,
                raw_wpm: row.raw_wpm,
                accuracy: row.accuracy,
                consistency: row.consistency,
                correct_keystrokes: decode("correct_keystrokes", row.correct_keystrokes)?,
                incorrect_keystrokes: decode("incorrect_keystrokes", row.incorrect_keystrokes)?,
                elapsed_ms: decode("elapsed_ms", row.elapsed_ms)?,
            },
            // `Json` is a newtype; `.0` unwraps the decoded value.
            breakdown: row.breakdown.0,
            timeline: row.timeline.0,
            created_at: row.created_at,
        })
    }
}

/// Inserts the result and, in the same statement, raises the name's leaderboard entry if the
/// new result is faster. One round trip, and atomic without an explicit transaction.
const SAVE_SQL: &str = "
WITH inserted AS (
    INSERT INTO results (id, mode, name, wpm, raw_wpm, accuracy, consistency,
                         correct_keystrokes, incorrect_keystrokes, elapsed_ms, breakdown,
                         timeline, created_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
    ON CONFLICT (id) DO NOTHING
    RETURNING id, mode, name, wpm, accuracy, created_at
), best AS (
    INSERT INTO leaderboard (mode, name, result_id, wpm, accuracy, created_at)
    SELECT mode, name, id, wpm, accuracy, created_at FROM inserted WHERE name IS NOT NULL
    ON CONFLICT (mode, name) DO UPDATE
        SET result_id = EXCLUDED.result_id,
            wpm = EXCLUDED.wpm,
            accuracy = EXCLUDED.accuracy,
            created_at = EXCLUDED.created_at
        WHERE EXCLUDED.wpm > leaderboard.wpm
)
SELECT EXISTS (SELECT 1 FROM inserted)";

impl ResultStore for PgStore {
    async fn save(&self, result: &TestResult) -> Result<(), StoreError> {
        let stats = &result.stats;
        // Postgres has no unsigned integers. Every stored u32 is bounded far below i32::MAX by
        // the scoring limits, so these casts cannot wrap.
        let inserted: bool = sqlx::query_scalar(SAVE_SQL)
            .bind(result.id)
            .bind(i16::from(result.mode.code()))
            .bind(result.name.as_ref().map(DisplayName::as_str))
            .bind(stats.wpm)
            .bind(stats.raw_wpm)
            .bind(stats.accuracy)
            .bind(stats.consistency)
            .bind(stats.correct_keystrokes as i32)
            .bind(stats.incorrect_keystrokes as i32)
            .bind(stats.elapsed_ms as i32)
            .bind(Json(&result.breakdown))
            .bind(Json(&result.timeline))
            .bind(result.created_at)
            .fetch_one(&self.pool)
            .await?;
        if inserted {
            Ok(())
        } else {
            Err(StoreError::Duplicate)
        }
    }

    async fn find(&self, id: Uuid) -> Result<Option<TestResult>, StoreError> {
        let row: Option<ResultRow> = sqlx::query_as(
            "SELECT id, mode, name, wpm, raw_wpm, accuracy, consistency, correct_keystrokes,
                    incorrect_keystrokes, elapsed_ms, breakdown, timeline, created_at
             FROM results WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(TestResult::try_from).transpose()?)
    }

    async fn leaderboard(
        &self,
        mode: TestMode,
        limit: u16,
    ) -> Result<Vec<LeaderboardEntry>, StoreError> {
        let rows: Vec<(String, f64, f64, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT name, wpm, accuracy, result_id, created_at FROM leaderboard
             WHERE mode = $1 ORDER BY wpm DESC, created_at LIMIT $2",
        )
        .bind(i16::from(mode.code()))
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(name, wpm, accuracy, result_id, created_at)| LeaderboardEntry {
                    name: DisplayName(name),
                    wpm,
                    accuracy,
                    result_id,
                    created_at,
                },
            )
            .collect())
    }
}

/// In-memory [`ResultStore`] with the same semantics as [`PgStore`], for handler tests.
#[cfg(test)]
pub mod memory {
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    use super::*;

    #[derive(Debug, Clone, Default)]
    pub struct MemoryStore {
        results: Arc<RwLock<HashMap<Uuid, TestResult>>>,
    }

    impl ResultStore for MemoryStore {
        async fn save(&self, result: &TestResult) -> Result<(), StoreError> {
            let mut results = self.results.write().expect("lock is not poisoned");
            if results.contains_key(&result.id) {
                return Err(StoreError::Duplicate);
            }
            results.insert(result.id, result.clone());
            Ok(())
        }

        async fn find(&self, id: Uuid) -> Result<Option<TestResult>, StoreError> {
            Ok(self
                .results
                .read()
                .expect("lock is not poisoned")
                .get(&id)
                .cloned())
        }

        async fn leaderboard(
            &self,
            mode: TestMode,
            limit: u16,
        ) -> Result<Vec<LeaderboardEntry>, StoreError> {
            let results = self.results.read().expect("lock is not poisoned");
            let mut best: HashMap<&str, &TestResult> = HashMap::new();
            for result in results.values().filter(|result| result.mode == mode) {
                let Some(name) = &result.name else { continue };
                let entry = best.entry(name.as_str()).or_insert(result);
                // Postgres keeps the first result on a tie, so the earliest one wins here too.
                let faster = result.stats.wpm > entry.stats.wpm;
                let earlier_tie =
                    result.stats.wpm == entry.stats.wpm && result.created_at < entry.created_at;
                if faster || earlier_tie {
                    *entry = result;
                }
            }
            let mut entries: Vec<LeaderboardEntry> = best
                .into_values()
                .map(|result| LeaderboardEntry {
                    name: result.name.clone().expect("filtered to named results"),
                    wpm: result.stats.wpm,
                    accuracy: result.stats.accuracy,
                    result_id: result.id,
                    created_at: result.created_at,
                })
                .collect();
            entries.sort_by(|a, b| {
                b.wpm
                    .total_cmp(&a.wpm)
                    .then_with(|| a.created_at.cmp(&b.created_at))
            });
            entries.truncate(usize::from(limit));
            Ok(entries)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_trimmed() {
        assert_eq!(
            DisplayName::parse("  ada  ").map(|name| name.0),
            Ok("ada".to_owned())
        );
    }

    #[test]
    fn name_limit_counts_chars_not_bytes() {
        assert!(DisplayName::parse(&"é".repeat(MAX_NAME_CHARS)).is_ok());
        assert_eq!(
            DisplayName::parse(&"a".repeat(MAX_NAME_CHARS + 1)),
            Err(InvalidName)
        );
    }

    #[test]
    fn rejects_blank_and_control_names() {
        assert_eq!(DisplayName::parse("   "), Err(InvalidName));
        assert_eq!(DisplayName::parse("a\nb"), Err(InvalidName));
    }

    #[test]
    fn rejects_invisible_and_bidi_names() {
        assert_eq!(DisplayName::parse("\u{200b}"), Err(InvalidName));
        assert_eq!(DisplayName::parse("ada\u{202e}"), Err(InvalidName));
        assert_eq!(DisplayName::parse("..."), Err(InvalidName));
    }

    #[test]
    fn accepts_unicode_letters_and_punctuation() {
        assert_eq!(
            DisplayName::parse("José O'Neil-2").map(|name| name.0),
            Ok("José O'Neil-2".to_owned())
        );
    }
}
