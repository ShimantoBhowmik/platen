//! Calls to the type-cast server, served from the same origin.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Whether a test is timed or counted in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeKind {
    /// Ends after `amount` seconds.
    Time,
    /// Ends after `amount` words.
    Words,
}

/// A test mode as the server names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSpec {
    /// Timed or counted.
    pub mode: ModeKind,
    /// Seconds or words.
    pub amount: u16,
}

impl ModeSpec {
    /// Every mode the server offers, in the order the rail shows them.
    pub const ALL: [ModeSpec; 8] = [
        ModeSpec {
            mode: ModeKind::Time,
            amount: 15,
        },
        ModeSpec {
            mode: ModeKind::Time,
            amount: 30,
        },
        ModeSpec {
            mode: ModeKind::Time,
            amount: 60,
        },
        ModeSpec {
            mode: ModeKind::Time,
            amount: 120,
        },
        ModeSpec {
            mode: ModeKind::Words,
            amount: 10,
        },
        ModeSpec {
            mode: ModeKind::Words,
            amount: 25,
        },
        ModeSpec {
            mode: ModeKind::Words,
            amount: 50,
        },
        ModeSpec {
            mode: ModeKind::Words,
            amount: 100,
        },
    ];

    /// Test length in milliseconds for a time test.
    pub fn limit_ms(self) -> Option<u32> {
        (self.mode == ModeKind::Time).then(|| u32::from(self.amount) * 1_000)
    }
}

/// A test issued by the server.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewTest {
    /// Signed token to send back with the result.
    pub token: String,
    /// The text to type.
    pub words: Vec<String>,
}

/// The verified scores.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Stats {
    /// Words per minute from correct words.
    pub wpm: f64,
    /// Words per minute from all final characters.
    pub raw_wpm: f64,
    /// Percent of character keys that were right.
    pub accuracy: f64,
    /// Steadiness of speed, 0 to 100.
    pub consistency: f64,
    /// Character keys that matched the text when typed.
    pub correct_keystrokes: u32,
    /// Character keys that did not match the text when typed.
    pub incorrect_keystrokes: u32,
    /// Duration the scores cover, in milliseconds.
    pub elapsed_ms: u32,
}

/// How the learner did on one key the text asked for.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct KeyStat {
    /// The key: a lowercase letter, or a space.
    pub key: char,
    /// Times it was typed right.
    pub hits: u32,
    /// Times something else was typed where it was expected.
    pub misses: u32,
}

/// A word left wrong.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MissedWord {
    /// The word in the text.
    pub expected: String,
    /// What was typed instead.
    pub typed: String,
}

/// What to practise next.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Breakdown {
    /// Every key the text asked for, weakest first.
    pub keys: Vec<KeyStat>,
    /// Words committed wrong, in typing order.
    pub missed_words: Vec<MissedWord>,
}

/// One second of the test.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct TimelinePoint {
    /// WPM from the start of the test to the end of this second.
    pub wpm: f64,
    /// Raw WPM of this second alone.
    pub raw_wpm: f64,
    /// Wrong keys in this second.
    pub errors: u32,
}

/// The server's verified result.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Report {
    /// The scores.
    pub stats: Stats,
    /// The mistake breakdown.
    pub breakdown: Breakdown,
    /// One point per scored second.
    pub timeline: Vec<TimelinePoint>,
}

#[derive(Serialize)]
struct Submission<'a> {
    token: &'a str,
    keys: &'a str,
    times: &'a [u32],
}

#[derive(Deserialize)]
struct ErrorBody {
    message: String,
}

/// Why a call failed, in words the learner can act on.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ApiError {
    /// The server could not be reached.
    #[error("the server could not be reached")]
    Network,
    /// The server refused the request and said why.
    #[error("{0}")]
    Refused(String),
}

impl From<gloo_net::Error> for ApiError {
    fn from(_: gloo_net::Error) -> Self {
        Self::Network
    }
}

async fn refused(response: gloo_net::http::Response) -> ApiError {
    match response.json::<ErrorBody>().await {
        Ok(body) => ApiError::Refused(body.message),
        Err(_) => ApiError::Refused(format!("the server answered {}", response.status())),
    }
}

/// Starts a test.
///
/// # Errors
///
/// Returns [`ApiError`] if the server is unreachable or refuses the mode.
pub async fn new_test(spec: ModeSpec) -> Result<NewTest, ApiError> {
    let response = Request::post("/api/tests").json(&spec)?.send().await?;
    if !response.ok() {
        return Err(refused(response).await);
    }
    Ok(response.json().await?)
}

/// Sends the keystroke log and returns the server's verified result.
///
/// # Errors
///
/// Returns [`ApiError`] if the server is unreachable or rejects the result.
pub async fn submit(token: &str, keys: &str, times: &[u32]) -> Result<Report, ApiError> {
    let body = Submission { token, keys, times };
    let response = Request::post("/api/results").json(&body)?.send().await?;
    if !response.ok() {
        return Err(refused(response).await);
    }
    Ok(response.json().await?)
}
