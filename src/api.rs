//! HTTP routes. Handlers are thin: parse at the boundary, call pure domain code, store.
//!
//! | Method | Path                       | Purpose                                    |
//! |--------|----------------------------|--------------------------------------------|
//! | POST   | `/api/tests`               | Start a test: signed token plus its words  |
//! | POST   | `/api/results`             | Submit keystrokes, get the verified result |
//! | GET    | `/api/results/{id}`        | Fetch a result, e.g. for a share link      |
//! | GET    | `/api/leaderboard`         | Best result per name for one mode          |
//! | GET    | `/health`                  | Liveness probe                             |

use std::path::Path;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::AppError;
use crate::mode::{ModeKind, ModeSpec, TestMode};
use crate::scoring::{self, Breakdown, Scored, TestStats, TimelinePoint};
use crate::store::{DisplayName, LeaderboardEntry, ResultStore, TestResult};
use crate::token::{TestTicket, TokenSigner};
use crate::words;

/// Largest accepted request body. A maximal submission is about 130 KiB of JSON.
const MAX_BODY_BYTES: usize = 256 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LEADERBOARD_LIMIT: u16 = 50;
const MAX_LEADERBOARD_LIMIT: u16 = 100;

/// Shared state for every handler. Cheap to clone: the store is a pool handle.
#[derive(Clone)]
pub struct AppState<S> {
    store: S,
    signer: TokenSigner,
}

impl<S: ResultStore> AppState<S> {
    /// Bundles a result store with the token signer.
    pub fn new(store: S, signer: TokenSigner) -> Self {
        Self { store, signer }
    }
}

/// `Json` extractor whose failures render as [`AppError`] JSON instead of plain text.
#[derive(FromRequest)]
#[from_request(via(Json), rejection(AppError))]
struct ApiJson<T>(T);

/// `Query` extractor whose failures render as [`AppError`] JSON.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
struct ApiQuery<T>(T);

/// `Path` extractor whose failures render as [`AppError`] JSON.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
struct ApiPath<T>(T);

/// Builds the application router with its middleware stack.
///
/// # Arguments
///
/// * `state` - Store and signer shared by all handlers
/// * `static_dir` - Built frontend to serve for every path the API does not handle; unknown
///   paths get its `index.html`
pub fn router<S: ResultStore>(state: AppState<S>, static_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/api/tests", post(create_test::<S>))
        .route("/api/results", post(submit_result::<S>))
        .route("/api/results/{id}", get(get_result::<S>))
        .route("/api/leaderboard", get(leaderboard::<S>))
        .route("/health", get(|| async { StatusCode::OK }))
        .with_state(state);
    let app = match static_dir {
        Some(dir) => api
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))),
        None => api,
    };
    app.layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
}

/// Response to `POST /api/tests`.
#[derive(Debug, Serialize)]
struct NewTest {
    /// Opaque signed token; send it back with the result.
    token: String,
    mode: ModeSpec,
    words: Vec<&'static str>,
}

async fn create_test<S: ResultStore>(
    State(state): State<AppState<S>>,
    ApiJson(spec): ApiJson<ModeSpec>,
) -> Result<Json<NewTest>, AppError> {
    let mode = TestMode::try_from(spec)?;
    let ticket = TestTicket {
        id: Uuid::new_v4(),
        issued_at_ms: unix_ms(Utc::now()),
        mode,
    };
    Ok(Json(NewTest {
        token: state.signer.sign(&ticket),
        mode: spec,
        words: words::generate(ticket.seed(), mode.word_count()),
    }))
}

/// Body of `POST /api/results`.
#[derive(Debug, Deserialize)]
struct Submission {
    token: String,
    /// Every key pressed, in order; `"\b"` is backspace.
    keys: String,
    /// Milliseconds since the first keystroke, one per key.
    times: Vec<u32>,
    /// Optional leaderboard name.
    name: Option<String>,
}

/// A stored result as the API presents it.
#[derive(Debug, Serialize)]
struct ResultView {
    id: Uuid,
    mode: ModeSpec,
    name: Option<DisplayName>,
    stats: TestStats,
    breakdown: Breakdown,
    timeline: Vec<TimelinePoint>,
    created_at: DateTime<Utc>,
}

impl From<TestResult> for ResultView {
    fn from(result: TestResult) -> Self {
        Self {
            id: result.id,
            mode: result.mode.spec(),
            name: result.name,
            stats: result.stats,
            breakdown: result.breakdown,
            timeline: result.timeline,
            created_at: result.created_at,
        }
    }
}

async fn submit_result<S: ResultStore>(
    State(state): State<AppState<S>>,
    ApiJson(submission): ApiJson<Submission>,
) -> Result<(StatusCode, Json<ResultView>), AppError> {
    let ticket = state.signer.verify(&submission.token)?;
    let name = submission
        .name
        .as_deref()
        .map(DisplayName::parse)
        .transpose()?;
    let now = Utc::now();
    // Scoring a maximal submission takes microseconds, far below the cost of moving it to the
    // blocking pool, so it runs inline on the async worker.
    let Scored {
        stats,
        breakdown,
        timeline,
    } = scoring::evaluate(&ticket, &submission.keys, &submission.times, unix_ms(now))?;
    let result = TestResult {
        id: ticket.id,
        mode: ticket.mode,
        name,
        stats,
        breakdown,
        timeline,
        created_at: now,
    };
    state.store.save(&result).await?;
    Ok((StatusCode::CREATED, Json(result.into())))
}

async fn get_result<S: ResultStore>(
    State(state): State<AppState<S>>,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<ResultView>, AppError> {
    let result = state.store.find(id).await?.ok_or(AppError::NotFound)?;
    Ok(Json(result.into()))
}

/// Query of `GET /api/leaderboard`, e.g. `?mode=time&amount=60&limit=10`.
#[derive(Debug, Deserialize)]
struct LeaderboardQuery {
    mode: ModeKind,
    amount: u16,
    limit: Option<u16>,
}

/// Response to `GET /api/leaderboard`. Entries are in rank order.
#[derive(Debug, Serialize)]
struct Leaderboard {
    mode: ModeSpec,
    entries: Vec<LeaderboardEntry>,
}

async fn leaderboard<S: ResultStore>(
    State(state): State<AppState<S>>,
    ApiQuery(query): ApiQuery<LeaderboardQuery>,
) -> Result<Json<Leaderboard>, AppError> {
    let spec = ModeSpec {
        mode: query.mode,
        amount: query.amount,
    };
    let mode = TestMode::try_from(spec)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LEADERBOARD_LIMIT)
        .clamp(1, MAX_LEADERBOARD_LIMIT);
    let entries = state.store.leaderboard(mode, limit).await?;
    Ok(Json(Leaderboard {
        mode: spec,
        entries,
    }))
}

fn unix_ms(time: DateTime<Utc>) -> u64 {
    // A clock before 1970 is a broken host; treating it as the epoch only makes tokens expire.
    u64::try_from(time.timestamp_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::store::memory::MemoryStore;

    const SECRET: &[u8] = b"test-secret";

    fn app() -> Router {
        router(
            AppState::new(MemoryStore::default(), TokenSigner::new(SECRET)),
            None,
        )
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |json| Body::from(json.to_string())))
            .expect("valid request");
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("JSON body")
        };
        (status, json)
    }

    /// A token for `mode` issued a minute ago, so any human timing is plausible.
    fn backdated_test(mode: TestMode) -> (String, Vec<&'static str>) {
        let ticket = TestTicket {
            id: Uuid::new_v4(),
            issued_at_ms: unix_ms(Utc::now()) - 60_000,
            mode,
        };
        let token = TokenSigner::new(SECRET).sign(&ticket);
        (token, words::generate(ticket.seed(), mode.word_count()))
    }

    /// Typing `text` with intervals cycling through 80..=200 ms.
    fn human_times(text: &str) -> Vec<u32> {
        let mut now = 0;
        (0..text.chars().count() as u32)
            .map(|index| {
                let time = now;
                now += 80 + (index * 37) % 121;
                time
            })
            .collect()
    }

    fn submission(token: &str, keys: &str, name: Option<&str>) -> Value {
        json!({ "token": token, "keys": keys, "times": human_times(keys), "name": name })
    }

    #[tokio::test]
    async fn create_test_returns_token_and_words() {
        let (status, body) = call(
            &app(),
            "POST",
            "/api/tests",
            Some(json!({"mode": "words", "amount": 25})),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["mode"], json!({"mode": "words", "amount": 25}));
        assert_eq!(body["words"].as_array().map(Vec::len), Some(25));
        assert_eq!(body["token"].as_str().map(str::len), Some(76));
    }

    #[tokio::test]
    async fn create_test_rejects_unsupported_mode() {
        let (status, body) = call(
            &app(),
            "POST",
            "/api/tests",
            Some(json!({"mode": "time", "amount": 17})),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_mode");
    }

    #[tokio::test]
    async fn malformed_json_gets_json_error() {
        let (status, body) = call(&app(), "POST", "/api/tests", Some(json!({"mode": 1}))).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_body");
    }

    #[tokio::test]
    async fn submit_then_fetch_then_rank() {
        let app = app();
        let (token, words) = backdated_test(TestMode::Words10);
        let keys = words.join(" ");

        let (status, created) = call(
            &app,
            "POST",
            "/api/results",
            Some(submission(&token, &keys, Some(" ada "))),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["name"], "ada");
        assert_eq!(created["stats"]["accuracy"], 100.0);
        assert_eq!(created["breakdown"]["missed_words"], json!([]));
        assert_eq!(created["breakdown"]["keys"][0]["misses"], 0);
        let timeline = created["timeline"].as_array().expect("timeline is a list");
        assert_eq!(
            timeline.last().map(|point| &point["wpm"]),
            Some(&created["stats"]["wpm"])
        );

        let id = created["id"].as_str().expect("id is a string");
        let (status, fetched) = call(&app, "GET", &format!("/api/results/{id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, created);

        let (status, board) =
            call(&app, "GET", "/api/leaderboard?mode=words&amount=10", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(board["entries"][0]["name"], "ada");
        assert_eq!(board["entries"][0]["result_id"], created["id"]);
        assert_eq!(board["entries"][0]["wpm"], created["stats"]["wpm"]);
    }

    #[tokio::test]
    async fn token_is_single_use() {
        let app = app();
        let (token, words) = backdated_test(TestMode::Words10);
        let body = submission(&token, &words.join(" "), None);

        let (first, _) = call(&app, "POST", "/api/results", Some(body.clone())).await;
        let (second, error) = call(&app, "POST", "/api/results", Some(body)).await;

        assert_eq!(first, StatusCode::CREATED);
        assert_eq!(second, StatusCode::CONFLICT);
        assert_eq!(error["error"], "already_submitted");
    }

    #[tokio::test]
    async fn cheating_submission_is_rejected_with_reason() {
        let (token, words) = backdated_test(TestMode::Words10);
        let keys = words.join(" ");
        let times: Vec<u32> = (0..keys.len() as u32).map(|index| index * 10).collect();
        let body = json!({ "token": token, "keys": keys, "times": times });

        let (status, error) = call(&app(), "POST", "/api/results", Some(body)).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["error"], "result_rejected");
        assert_eq!(error["reason"], "too_fast");
    }

    #[tokio::test]
    async fn forged_token_is_rejected() {
        let forged = TokenSigner::new(b"attacker").sign(&TestTicket {
            id: Uuid::new_v4(),
            issued_at_ms: 0,
            mode: TestMode::Words10,
        });
        let body = submission(&forged, "a", None);

        let (status, error) = call(&app(), "POST", "/api/results", Some(body)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "invalid_token");
    }

    #[tokio::test]
    async fn invalid_name_is_rejected() {
        let (token, words) = backdated_test(TestMode::Words10);
        let body = submission(&token, &words.join(" "), Some("   "));

        let (status, error) = call(&app(), "POST", "/api/results", Some(body)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "invalid_name");
    }

    #[tokio::test]
    async fn unknown_result_is_not_found() {
        let uri = format!("/api/results/{}", Uuid::new_v4());
        let (status, error) = call(&app(), "GET", &uri, None).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"], "not_found");
    }

    #[tokio::test]
    async fn bad_result_id_gets_json_error() {
        let (status, error) = call(&app(), "GET", "/api/results/nope", None).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "invalid_path");
    }

    #[tokio::test]
    async fn leaderboard_keeps_each_names_best() {
        let app = app();
        let mut wpm_by_run = Vec::new();
        for (name, slowdown_keys) in [("ada", 0), ("ada", 10), ("bob", 20)] {
            let (token, words) = backdated_test(TestMode::Words10);
            // Corrected typos before the text add time, so each run is slower than the last.
            let keys = format!("{}{}", "x\u{8}".repeat(slowdown_keys / 2), words.join(" "));
            let (status, created) = call(
                &app,
                "POST",
                "/api/results",
                Some(submission(&token, &keys, Some(name))),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
            wpm_by_run.push(created["stats"]["wpm"].clone());
        }

        let (_, board) = call(
            &app,
            "GET",
            "/api/leaderboard?mode=words&amount=10&limit=10",
            None,
        )
        .await;

        let ranked: Vec<(&Value, &Value)> = board["entries"]
            .as_array()
            .expect("entries array")
            .iter()
            .map(|entry| (&entry["name"], &entry["wpm"]))
            .collect();
        let (ada, bob) = (json!("ada"), json!("bob"));
        assert_eq!(ranked, [(&ada, &wpm_by_run[0]), (&bob, &wpm_by_run[2])]);
    }

    #[tokio::test]
    async fn health_is_ok() {
        let (status, _) = call(&app(), "GET", "/health", None).await;
        assert_eq!(status, StatusCode::OK);
    }
}
