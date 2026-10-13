//! Server-side scoring of a keystroke log.
//!
//! The client never reports its own WPM. It sends every keystroke with a timestamp, and the
//! server replays them against the text it regenerates from the token. Scoring follows the
//! common typing-test rules: space moves to the next word, backspace may return to the
//! previous word, and WPM counts the characters of correctly typed words (plus their spaces)
//! divided by five per minute.
//!
//! The same replay also produces a [`Breakdown`] of the learner's mistakes: how often each key
//! of the text was hit or missed, and which words were committed wrong. It also produces a
//! per-second [`TimelinePoint`] series for the result's speed chart.
//!
//! Everything here is pure: it takes the clock as an argument, so it is fully unit-testable.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::mode::{MAX_KEYSTROKE_WPM, TestMode};
use crate::token::TestTicket;
use crate::words;

/// The key code clients send for backspace (JSON string escape `\b`).
pub const BACKSPACE: char = '\u{8}';
/// Upper bound on keystrokes per submission. A 120 s test at the speed cap needs about 4,200.
pub const MAX_KEYSTROKES: usize = 10_000;
/// How long a token stays valid after it is issued.
pub const TOKEN_TTL_MS: u64 = 15 * 60 * 1_000;
/// Clock rounding allowance when checking that the test really took as long as claimed.
const CLOCK_TOLERANCE_MS: u64 = 500;
/// Keys a time test may record after its limit, from the race between the client's timer and
/// its input handler. They are ignored, not scored.
const LATE_KEY_TOLERANCE_MS: u32 = 100;
/// Minimum number of keystroke intervals before the rhythm check applies.
const MIN_RHYTHM_SAMPLES: u32 = 20;
/// Human key intervals vary a lot (coefficient of variation is usually above 0.3). A steady
/// machine-like rhythm below this is rejected.
const MIN_HUMAN_INTERVAL_CV: f64 = 0.1;

/// A verified test result.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TestStats {
    /// Words per minute from correctly typed words.
    pub wpm: f64,
    /// Words per minute from every character in the final text, right or wrong. Characters
    /// deleted with backspace do not count.
    pub raw_wpm: f64,
    /// Percentage of character keystrokes that were correct when typed.
    pub accuracy: f64,
    /// Speed steadiness from 0 to 100, from how much the per-second raw speed varies.
    pub consistency: f64,
    /// Character keystrokes that matched the text when typed.
    pub correct_keystrokes: u32,
    /// Character keystrokes that did not match the text when typed.
    pub incorrect_keystrokes: u32,
    /// Duration the scores are computed over, in milliseconds.
    pub elapsed_ms: u32,
}

/// How the learner did on one key of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyStat {
    /// The key the text asked for: a lowercase letter, or `' '` for the space bar.
    pub key: char,
    /// Keystrokes that typed this key where the text expected it.
    pub hits: u32,
    /// Keystrokes that typed something else where the text expected this key. A space that
    /// commits a mistyped full-length word counts as a missed space.
    pub misses: u32,
}

/// A word committed with errors, as the learner left it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissedWord {
    /// The word in the text.
    pub expected: String,
    /// What the learner typed instead.
    pub typed: String,
}

/// What the learner should practise next.
///
/// Corrected typos still count in `keys`, since the key was missed when typed. `missed_words`
/// holds only the words left wrong. Keystrokes past the end of a word have no expected key and
/// appear only in [`TestStats::incorrect_keystrokes`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Breakdown {
    /// Every key the text asked for, weakest first: highest miss rate, then most misses.
    pub keys: Vec<KeyStat>,
    /// Committed words that differ from the text, in typing order.
    pub missed_words: Vec<MissedWord>,
}

/// The typing in one second of the test, for the result's speed chart.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimelinePoint {
    /// WPM from the start of the test to the end of this second, counted like
    /// [`TestStats::wpm`]. The last point runs to the end of the test, so it equals the test's
    /// WPM.
    pub wpm: f64,
    /// Raw WPM of the character keys typed in this second alone, rounded to a whole number.
    /// These values are what [`TestStats::consistency`] measures.
    pub raw_wpm: f64,
    /// Character keystrokes in this second that did not match the text.
    pub errors: u32,
}

/// The full outcome of scoring a submission.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    /// The verified speed and accuracy numbers.
    pub stats: TestStats,
    /// The mistake breakdown for the learner.
    pub breakdown: Breakdown,
    /// One point per scored second of the test, in order.
    pub timeline: Vec<TimelinePoint>,
}

/// The keystrokes of one second, recorded while the replay runs.
///
/// Only seconds that contain keys get a mark, so memory is bounded by the keystroke count
/// even before the timing checks have bounded the length of the test.
#[derive(Debug, Clone, Copy, Default)]
struct SecondMark {
    /// Zero-based second of the test, from [`second_of`].
    second: u32,
    /// Keystrokes in this second that the score counts: character keys, but not backspaces or
    /// ignored spaces.
    chars: u32,
    /// Character keys in this second that did not match the text.
    errors: u32,
    /// Characters that counted toward WPM when this second's first key arrived. That is also
    /// the count at the end of every earlier second without keys.
    correct_chars_before: u32,
}

/// The second a keystroke belongs to. A key at exactly 1,000 ms closes the first second.
fn second_of(time_ms: u32) -> u32 {
    time_ms.saturating_sub(1) / 1_000
}

/// Hits and misses for one expected key, indexed by its ASCII byte in [`Replay`].
#[derive(Debug, Clone, Copy, Default)]
struct KeyTally {
    hits: u32,
    misses: u32,
}

impl KeyTally {
    fn attempts(self) -> u32 {
        self.hits + self.misses
    }
}

/// Orders tallies weakest first. Miss rates are compared by cross-multiplying, which is exact
/// where float division could tie wrongly.
fn weaker_first(a: &KeyStat, b: &KeyStat) -> Ordering {
    let attempts = |stat: &KeyStat| u64::from(stat.hits + stat.misses);
    (u64::from(b.misses) * attempts(a))
        .cmp(&(u64::from(a.misses) * attempts(b)))
        .then(b.misses.cmp(&a.misses))
        .then(a.key.cmp(&b.key))
}

/// Why a submission was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// No character keys were typed.
    #[error("the submission contains no typed characters")]
    Empty,
    /// More than [`MAX_KEYSTROKES`] keys.
    #[error("the submission has more than {MAX_KEYSTROKES} keystrokes")]
    TooManyKeystrokes,
    /// `keys` and `times` do not pair up one to one.
    #[error("keys and times must have the same length")]
    LengthMismatch,
    /// A timestamp is earlier than the one before it.
    #[error("keystroke times must not decrease")]
    TimesNotMonotonic,
    /// A time test has a key well after its limit.
    #[error("a keystroke falls after the end of the timed test")]
    ExceedsTimeLimit,
    /// A word test stopped before its last word.
    #[error("the word test was not finished")]
    Incomplete,
    /// Keys follow the end of a word test.
    #[error("keystrokes continue after the test ended")]
    ExtraInput,
    /// Keystroke rate above [`MAX_KEYSTROKE_WPM`].
    #[error("the keystroke rate is above the {MAX_KEYSTROKE_WPM} WPM limit")]
    TooFast,
    /// Key intervals are too uniform for a human.
    #[error("the keystroke rhythm is too regular to be human")]
    InhumanRhythm,
    /// Less wall-clock time passed since the token was issued than the test claims to take.
    #[error("the result arrived before the test could have finished")]
    SubmittedTooEarly,
    /// The token is older than [`TOKEN_TTL_MS`].
    #[error("the test token has expired")]
    Expired,
}

/// Scores a submission and rejects anything a human could not have typed.
///
/// # Arguments
///
/// * `ticket` - The verified token contents
/// * `keys` - Every key pressed, in order; [`BACKSPACE`] marks a backspace
/// * `times_ms` - Milliseconds since the first keystroke, one per key
/// * `now_ms` - Current Unix time in milliseconds
///
/// # Returns
///
/// The verified [`TestStats`] and the learner's mistake [`Breakdown`].
///
/// # Errors
///
/// Returns a [`Rejection`] that names the first rule the submission broke.
///
/// # Examples
///
/// ```ignore
/// let scored = evaluate(&ticket, "the be", &[0, 150, 300, 450, 600, 750], now_ms)?;
/// assert!(scored.stats.wpm > 0.0);
/// ```
pub fn evaluate(
    ticket: &TestTicket,
    keys: &str,
    times_ms: &[u32],
    now_ms: u64,
) -> Result<Scored, Rejection> {
    if times_ms.len() > MAX_KEYSTROKES {
        return Err(Rejection::TooManyKeystrokes);
    }
    if keys.chars().count() != times_ms.len() {
        return Err(Rejection::LengthMismatch);
    }
    let since_issue_ms = now_ms.saturating_sub(ticket.issued_at_ms);
    if since_issue_ms > TOKEN_TTL_MS {
        return Err(Rejection::Expired);
    }

    let targets = words::generate(ticket.seed(), ticket.mode.word_count());
    let replay = Replay::run(ticket.mode, &targets, keys, times_ms)?;
    if u64::from(replay.elapsed_ms) > since_issue_ms + CLOCK_TOLERANCE_MS {
        return Err(Rejection::SubmittedTooEarly);
    }
    replay.into_scored(ticket.mode.time_limit_ms().is_none())
}

/// Expands the sparse per-second marks of a replay into one point per scored second.
///
/// A word test also scores its last partial second when it is at least half a second long; a
/// time test always ends on a whole second. Keys in an unscored partial second count only
/// toward the WPM of the last point.
///
/// # Arguments
///
/// * `marks` - The replay's marks, in ascending order of `second`
/// * `final_correct_chars` - Characters that count toward WPM at the end of the test
/// * `elapsed_ms` - The scored duration of the test. It must already be bounded, because the
///   result has one point per second.
/// * `scores_partial_second` - Whether a final partial second gets its own point
///
/// # Returns
///
/// The points, empty when the test lasted less than one scored second.
fn timeline(
    marks: &[SecondMark],
    final_correct_chars: u32,
    elapsed_ms: u32,
    scores_partial_second: bool,
) -> Vec<TimelinePoint> {
    let whole_seconds = elapsed_ms / 1_000;
    let tail_ms = elapsed_ms % 1_000;
    let has_tail = scores_partial_second && tail_ms >= 500;
    let points = whole_seconds + u32::from(has_tail);
    // Index of the first mark not yet consumed. Marks ascend, so one forward pass pairs them.
    let mut next = 0;
    (0..points)
        .map(|second| {
            let here = marks.get(next).filter(|mark| mark.second == second);
            if here.is_some() {
                next += 1;
            }
            let (correct_chars, through_ms) = if second + 1 == points {
                (final_correct_chars, elapsed_ms)
            } else {
                // The next second with keys starts from what this second ended with.
                let correct_chars = marks
                    .get(next)
                    .map_or(final_correct_chars, |mark| mark.correct_chars_before);
                (correct_chars, (second + 1) * 1_000)
            };
            let span_ms = if second < whole_seconds {
                1_000
            } else {
                tail_ms
            };
            TimelinePoint {
                wpm: round2(words_per_minute(correct_chars, through_ms)),
                raw_wpm: words_per_minute(here.map_or(0, |mark| mark.chars), span_ms).round(),
                errors: here.map_or(0, |mark| mark.errors),
            }
        })
        .collect()
}

/// Steadiness from the raw speed of each second of the test.
///
/// # Arguments
///
/// * `timeline` - One point per scored second, from [`timeline`]
///
/// # Returns
///
/// A score from 0 to 100; 0 when there is no scored second or no typing in them.
fn consistency(timeline: &[TimelinePoint]) -> f64 {
    let mut spread = Spread::default();
    timeline.iter().for_each(|point| spread.push(point.raw_wpm));
    // Also covers an empty timeline, where the mean stays 0.
    if spread.mean == 0.0 {
        return 0.0;
    }
    let cv = spread.population_std_dev() / spread.mean;
    // About 100 * (1 - cv) for small variation, tapering smoothly toward 0 instead of
    // clipping at cv = 1.
    round2(100.0 * (1.0 - (cv + cv.powi(3) / 3.0 + cv.powi(5) / 5.0).tanh()))
}

/// Running mean and variance (Welford's one-pass algorithm).
#[derive(Debug, Default)]
struct Spread {
    count: u32,
    mean: f64,
    sum_squared_deviation: f64,
}

impl Spread {
    fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / f64::from(self.count);
        self.sum_squared_deviation += delta * (value - self.mean);
    }

    fn population_std_dev(&self) -> f64 {
        (self.sum_squared_deviation / f64::from(self.count)).sqrt()
    }

    /// Standard deviation divided by mean. `None` with fewer than two samples.
    fn coefficient_of_variation(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        if self.mean == 0.0 {
            return Some(0.0);
        }
        Some(self.population_std_dev() / self.mean)
    }
}

/// The state of a keystroke replay.
struct Replay<'t> {
    targets: &'t [&'static str],
    /// Everything currently typed, with exactly one space between words.
    typed: String,
    /// Byte offset in `typed` where the current word starts.
    word_start: usize,
    word_index: usize,
    /// Characters (not bytes) in the current word, so a non-ASCII key cannot shift positions.
    word_chars: usize,
    /// Characters credited toward WPM by committed correct words, each with its space.
    committed_correct_chars: u32,
    correct_keystrokes: u32,
    incorrect_keystrokes: u32,
    /// Per expected key, indexed by its ASCII byte. The text is ASCII, so a fixed array is an
    /// exact, allocation-free map.
    key_tallies: [KeyTally; 128],
    /// Gaps between keystrokes, for the anti-cheat rhythm check.
    intervals: Spread,
    /// Keystrokes per second, for the timeline.
    seconds: Vec<SecondMark>,
    elapsed_ms: u32,
    /// Whether a word test reached its end. Always `false` for a time test.
    finished: bool,
    /// Whether a space ended the word test. That space is a final character but is not
    /// stored in `typed`.
    ended_on_space: bool,
}

impl<'t> Replay<'t> {
    fn run(
        mode: TestMode,
        targets: &'t [&'static str],
        keys: &str,
        times_ms: &[u32],
    ) -> Result<Self, Rejection> {
        let mut replay = Self {
            targets,
            typed: String::with_capacity(keys.len()),
            word_start: 0,
            word_index: 0,
            word_chars: 0,
            committed_correct_chars: 0,
            correct_keystrokes: 0,
            incorrect_keystrokes: 0,
            key_tallies: [KeyTally::default(); 128],
            intervals: Spread::default(),
            seconds: Vec::new(),
            elapsed_ms: 0,
            finished: false,
            ended_on_space: false,
        };
        let time_limit_ms = mode.time_limit_ms();
        let mut previous_ms = 0;
        let mut finished = false;

        for (index, (key, time_ms)) in keys.chars().zip(times_ms.iter().copied()).enumerate() {
            if finished {
                return Err(Rejection::ExtraInput);
            }
            if time_ms < previous_ms {
                return Err(Rejection::TimesNotMonotonic);
            }
            if let Some(limit_ms) = time_limit_ms
                && time_ms > limit_ms
            {
                if time_ms > limit_ms + LATE_KEY_TOLERANCE_MS {
                    return Err(Rejection::ExceedsTimeLimit);
                }
                continue;
            }
            if index > 0 {
                replay.intervals.push(f64::from(time_ms - previous_ms));
            }
            previous_ms = time_ms;

            let second = second_of(time_ms);
            // `is_none_or` is true for an empty list or a mark from an earlier second.
            if replay
                .seconds
                .last()
                .is_none_or(|mark| mark.second != second)
            {
                let correct_chars_before = replay.correct_chars();
                replay.seconds.push(SecondMark {
                    second,
                    correct_chars_before,
                    ..SecondMark::default()
                });
            }

            finished = match key {
                BACKSPACE => {
                    replay.backspace();
                    false
                }
                ' ' => replay.space(),
                character => replay.character(character),
            };
            if finished {
                // Time tests size their text so that only typing above the speed cap can reach
                // its end.
                if time_limit_ms.is_some() {
                    return Err(Rejection::TooFast);
                }
                replay.elapsed_ms = time_ms;
                replay.ended_on_space = key == ' ';
            }
        }

        match time_limit_ms {
            Some(limit_ms) => replay.elapsed_ms = limit_ms,
            None if !finished => return Err(Rejection::Incomplete),
            None => {}
        }
        replay.finished = finished;
        Ok(replay)
    }

    fn current_word(&self) -> &str {
        &self.typed[self.word_start..]
    }

    fn target(&self) -> &'static str {
        self.targets[self.word_index]
    }

    fn is_last_word(&self) -> bool {
        self.word_index + 1 == self.targets.len()
    }

    /// Counts a character keystroke, crediting it to the key the text expected, if any.
    fn record(&mut self, correct: bool, expected: Option<u8>) {
        let tally = expected.map(|byte| &mut self.key_tallies[usize::from(byte)]);
        if correct {
            self.correct_keystrokes += 1;
            if let Some(tally) = tally {
                tally.hits += 1;
            }
        } else {
            self.incorrect_keystrokes += 1;
            if let Some(tally) = tally {
                tally.misses += 1;
            }
        }
        // Keys are recorded only inside the replay loop, after it marked their second.
        if let Some(mark) = self.seconds.last_mut() {
            mark.chars += 1;
            mark.errors += u32::from(!correct);
        }
    }

    /// Types one character. Returns `true` when it completes the last word correctly.
    fn character(&mut self, character: char) -> bool {
        let expected = self.target().as_bytes().get(self.word_chars).copied();
        self.record(
            expected.is_some_and(|byte| char::from(byte) == character),
            expected,
        );
        self.typed.push(character);
        self.word_chars += 1;
        self.is_last_word() && self.current_word() == self.target()
    }

    /// Commits the current word. Returns `true` when it was the last word.
    fn space(&mut self) -> bool {
        if self.word_chars == 0 {
            // Typing tests ignore a space before any letter of a word.
            return false;
        }
        // A premature space misses the next letter; after a full word it is the space itself.
        let expected = self
            .target()
            .as_bytes()
            .get(self.word_chars)
            .copied()
            .unwrap_or(b' ');
        let correct = self.current_word() == self.target();
        self.record(correct, Some(expected));
        if self.is_last_word() {
            return true;
        }
        if correct {
            // A correct word is locked against backspace, so this credit is never taken back.
            // Word lengths are tiny, so the cast cannot truncate.
            self.committed_correct_chars += self.word_chars as u32 + 1;
        }
        self.typed.push(' ');
        self.word_start = self.typed.len();
        self.word_index += 1;
        self.word_chars = 0;
        false
    }

    /// Deletes a character, or steps back into the previous word if it was typed wrong.
    /// A correct word is locked, as in common typing tests, so re-committing it cannot pad
    /// accuracy.
    fn backspace(&mut self) {
        if self.word_chars > 0 {
            self.typed.pop();
            self.word_chars -= 1;
            return;
        }
        if self.word_index == 0 {
            return;
        }
        let previous_end = self.word_start - 1;
        let previous_start = self.typed[..previous_end]
            .rfind(' ')
            .map_or(0, |space| space + 1);
        let previous = &self.typed[previous_start..previous_end];
        if previous == self.targets[self.word_index - 1] {
            return;
        }
        self.word_chars = previous.chars().count();
        // Drop the separating space.
        self.typed.pop();
        self.word_index -= 1;
        self.word_start = previous_start;
    }

    /// Committed words paired with their targets. A finished word test also commits its last
    /// word, which has no trailing space in `typed`.
    fn committed_words(&self) -> impl Iterator<Item = (&str, &'static str)> {
        let last = self.finished.then(|| (self.current_word(), self.target()));
        self.typed[..self.word_start]
            .split_terminator(' ')
            .zip(self.targets.iter().copied())
            .chain(last)
    }

    /// Characters that count toward WPM: each correctly committed word plus its space, and
    /// the correct prefix of the word in progress. The last word of a finished word test ends
    /// the test without a space, so it earns no space credit, and only if it is right.
    fn correct_chars(&self) -> u32 {
        let current = self.current_word();
        let credited = if self.finished {
            current == self.target()
        } else {
            self.target().starts_with(current)
        };
        // The current word is at most one test word plus a few extra letters.
        self.committed_correct_chars + if credited { current.len() as u32 } else { 0 }
    }

    fn breakdown(&self) -> Breakdown {
        let mut keys: Vec<KeyStat> = self
            .key_tallies
            .iter()
            .enumerate()
            .filter(|(_, tally)| tally.attempts() > 0)
            .map(|(byte, tally)| KeyStat {
                // Indices are below 128, so every one is an ASCII byte.
                key: char::from(byte as u8),
                hits: tally.hits,
                misses: tally.misses,
            })
            .collect();
        keys.sort_unstable_by(weaker_first);
        let missed_words = self
            .committed_words()
            .filter(|(typed, target)| typed != target)
            .map(|(typed, target)| MissedWord {
                expected: target.to_owned(),
                typed: typed.to_owned(),
            })
            .collect();
        Breakdown { keys, missed_words }
    }

    /// Characters left in the final text, right or wrong, including the spaces between words.
    fn final_chars(&self) -> u32 {
        // Each character came from a keystroke, and keystrokes are bounded by MAX_KEYSTROKES.
        self.typed.chars().count() as u32 + u32::from(self.ended_on_space)
    }

    /// Applies the speed and rhythm checks, then computes the scores.
    ///
    /// `scores_partial_second` is whether a final partial second gets its own timeline point.
    /// The caller must already have bounded `elapsed_ms`, which sets the timeline's length.
    fn into_scored(self, scores_partial_second: bool) -> Result<Scored, Rejection> {
        let typed_keystrokes = self.correct_keystrokes + self.incorrect_keystrokes;
        if typed_keystrokes == 0 {
            return Err(Rejection::Empty);
        }
        if self.elapsed_ms == 0 {
            return Err(Rejection::TooFast);
        }
        if words_per_minute(typed_keystrokes, self.elapsed_ms) > f64::from(MAX_KEYSTROKE_WPM) {
            return Err(Rejection::TooFast);
        }
        let interval_cv = self.intervals.coefficient_of_variation();
        if self.intervals.count >= MIN_RHYTHM_SAMPLES
            && interval_cv.is_some_and(|cv| cv < MIN_HUMAN_INTERVAL_CV)
        {
            return Err(Rejection::InhumanRhythm);
        }

        let correct_chars = self.correct_chars();
        let timeline = timeline(
            &self.seconds,
            correct_chars,
            self.elapsed_ms,
            scores_partial_second,
        );
        let stats = TestStats {
            wpm: round2(words_per_minute(correct_chars, self.elapsed_ms)),
            raw_wpm: round2(words_per_minute(self.final_chars(), self.elapsed_ms)),
            accuracy: round2(
                f64::from(self.correct_keystrokes) * 100.0 / f64::from(typed_keystrokes),
            ),
            consistency: consistency(&timeline),
            correct_keystrokes: self.correct_keystrokes,
            incorrect_keystrokes: self.incorrect_keystrokes,
            elapsed_ms: self.elapsed_ms,
        };
        Ok(Scored {
            stats,
            breakdown: self.breakdown(),
            timeline,
        })
    }
}

/// Words of five characters per minute. The timeline and the stats share it, so the last
/// point of the timeline and the test's WPM are the same number.
fn words_per_minute(chars: u32, ms: u32) -> f64 {
    f64::from(chars) / 5.0 / (f64::from(ms) / 60_000.0)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    const ISSUED_AT_MS: u64 = 1_760_000_000_000;

    fn ticket(mode: TestMode) -> TestTicket {
        TestTicket {
            id: Uuid::from_u128(42),
            issued_at_ms: ISSUED_AT_MS,
            mode,
        }
    }

    fn targets(mode: TestMode) -> Vec<&'static str> {
        words::generate(ticket(mode).seed(), mode.word_count())
    }

    /// Keys for typing `words` perfectly, joined by spaces, with no trailing space.
    fn perfect_keys(words: &[&str]) -> String {
        words.join(" ")
    }

    /// Human-like timestamps: intervals cycle through 80..=200 ms.
    fn human_times(count: usize) -> Vec<u32> {
        let mut now = 0;
        (0..count)
            .map(|index| {
                let time = now;
                now += 80 + (index as u32 * 37) % 121;
                time
            })
            .collect()
    }

    fn score_late(mode: TestMode, keys: &str, times: &[u32]) -> Result<Scored, Rejection> {
        evaluate(&ticket(mode), keys, times, ISSUED_AT_MS + 10 * 60 * 1_000)
    }

    fn evaluate_late(mode: TestMode, keys: &str, times: &[u32]) -> Result<TestStats, Rejection> {
        score_late(mode, keys, times).map(|scored| scored.stats)
    }

    #[test]
    fn perfect_word_test_scores_full_accuracy() {
        let keys = perfect_keys(&targets(TestMode::Words10));
        let times = human_times(keys.len());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let elapsed_ms = *times.last().expect("non-empty");
        let expected_wpm = round2(keys.len() as f64 / 5.0 / (f64::from(elapsed_ms) / 60_000.0));
        assert_eq!(stats.wpm, expected_wpm);
        assert_eq!(stats.raw_wpm, expected_wpm);
        assert_eq!(stats.accuracy, 100.0);
        assert_eq!(stats.correct_keystrokes as usize, keys.len());
        assert_eq!(stats.incorrect_keystrokes, 0);
        assert_eq!(stats.elapsed_ms, elapsed_ms);
    }

    #[test]
    fn corrected_typo_lowers_accuracy_not_wpm() {
        let words = targets(TestMode::Words10);
        let mut keys = String::from("q\u{8}");
        keys.push_str(&perfect_keys(&words));
        let times = human_times(keys.chars().count());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let typed = keys.len() - 2;
        assert_eq!(stats.correct_keystrokes as usize, typed);
        assert_eq!(stats.incorrect_keystrokes, 1);
        assert_eq!(
            stats.accuracy,
            round2(typed as f64 * 100.0 / (typed + 1) as f64)
        );
    }

    #[test]
    fn wrong_word_earns_no_wpm_credit() {
        let words = targets(TestMode::Words10);
        let mut typed: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        typed[0] = "x".repeat(words[0].len());
        let keys = typed.join(" ");
        let times = human_times(keys.len());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let elapsed_minutes = f64::from(*times.last().expect("non-empty")) / 60_000.0;
        let credited = keys.len() - words[0].len() - 1;
        assert_eq!(stats.wpm, round2(credited as f64 / 5.0 / elapsed_minutes));
        assert_eq!(stats.incorrect_keystrokes as usize, words[0].len() + 1);
    }

    #[test]
    fn backspace_returns_to_previous_word() {
        let words = targets(TestMode::Words10);
        // Commit the first word with a typo, go back, fix it, and finish the test.
        let mut keys = format!("{}x \u{8}\u{8} ", words[0]);
        keys.push_str(&perfect_keys(&words[1..]));
        let times = human_times(keys.chars().count());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        assert_eq!(stats.incorrect_keystrokes, 2);
        let elapsed_minutes = f64::from(*times.last().expect("non-empty")) / 60_000.0;
        let credited = perfect_keys(&words).len();
        assert_eq!(stats.wpm, round2(credited as f64 / 5.0 / elapsed_minutes));
    }

    #[test]
    fn time_test_uses_full_duration_and_counts_partial_word() {
        let words = targets(TestMode::Time15);
        let keys = format!("{} {}", words[0], &words[1][..1]);
        let times = human_times(keys.len());

        let stats = evaluate_late(TestMode::Time15, &keys, &times).expect("valid submission");

        assert_eq!(stats.elapsed_ms, 15_000);
        assert_eq!(stats.wpm, round2(keys.len() as f64 / 5.0 / 0.25));
    }

    #[test]
    fn rejects_unfinished_word_test() {
        let keys = perfect_keys(&targets(TestMode::Words10)[..5]);
        let times = human_times(keys.len());
        assert_eq!(
            evaluate_late(TestMode::Words10, &keys, &times),
            Err(Rejection::Incomplete)
        );
    }

    #[test]
    fn rejects_input_after_word_test_ends() {
        let keys = perfect_keys(&targets(TestMode::Words10)) + "x";
        let times = human_times(keys.len());
        assert_eq!(
            evaluate_late(TestMode::Words10, &keys, &times),
            Err(Rejection::ExtraInput)
        );
    }

    #[test]
    fn rejects_keystroke_after_time_limit() {
        assert_eq!(
            evaluate_late(TestMode::Time15, "ab", &[0, 15_101]),
            Err(Rejection::ExceedsTimeLimit)
        );
    }

    #[test]
    fn ignores_keystroke_just_after_time_limit() {
        let word = targets(TestMode::Time15)[0];
        let keys = format!("{word}x");
        let mut times = human_times(word.len());
        times.push(15_050);

        let stats = evaluate_late(TestMode::Time15, &keys, &times).expect("valid submission");

        assert_eq!(stats.incorrect_keystrokes, 0);
        assert_eq!(stats.correct_keystrokes as usize, word.len());
    }

    #[test]
    fn one_letter_words_under_speed_cap_do_not_run_out() {
        // 88 "words" of one letter and a space in under 15 s: 140 raw WPM.
        let keys = "x ".repeat(88);
        let times: Vec<u32> = (0..keys.len() as u32)
            .map(|index| index * 70 + index % 7 * 9)
            .collect();

        let stats = evaluate_late(TestMode::Time15, &keys, &times).expect("valid submission");

        assert_eq!(stats.raw_wpm, 140.8);
    }

    #[test]
    fn correct_word_is_locked_against_backspace() {
        let word = targets(TestMode::Time60)[0];
        let honest = format!("{word} ");
        // Re-committing a correct word over and over would pad accuracy if backspace could
        // re-open it.
        let padded = format!("{word}{}", " \u{8}".repeat(50));

        let honest_stats = evaluate_late(TestMode::Time60, &honest, &human_times(honest.len()))
            .expect("valid submission");
        let padded_stats = evaluate_late(
            TestMode::Time60,
            &padded,
            &human_times(padded.chars().count()),
        )
        .expect("valid submission");

        assert_eq!(
            padded_stats.correct_keystrokes,
            honest_stats.correct_keystrokes
        );
        assert_eq!(padded_stats.incorrect_keystrokes, 0);
    }

    #[test]
    fn rejects_decreasing_times() {
        assert_eq!(
            evaluate_late(TestMode::Time15, "ab", &[100, 50]),
            Err(Rejection::TimesNotMonotonic)
        );
    }

    #[test]
    fn rejects_length_mismatch_counting_chars_not_bytes() {
        assert_eq!(
            evaluate_late(TestMode::Time15, "é", &[0]).map(|_| ()),
            Ok(())
        );
        assert_eq!(
            evaluate_late(TestMode::Time15, "é", &[0, 1]),
            Err(Rejection::LengthMismatch)
        );
    }

    #[test]
    fn rejects_empty_submission() {
        assert_eq!(
            evaluate_late(TestMode::Time15, "", &[]),
            Err(Rejection::Empty)
        );
        assert_eq!(
            evaluate_late(TestMode::Time15, "  ", &[0, 100]),
            Err(Rejection::Empty)
        );
    }

    #[test]
    fn rejects_too_many_keystrokes() {
        let keys = "a".repeat(MAX_KEYSTROKES + 1);
        let times = vec![0; MAX_KEYSTROKES + 1];
        assert_eq!(
            evaluate_late(TestMode::Time120, &keys, &times),
            Err(Rejection::TooManyKeystrokes)
        );
    }

    #[test]
    fn rejects_superhuman_speed() {
        let keys = perfect_keys(&targets(TestMode::Words10));
        let times: Vec<u32> = (0..keys.len() as u32).map(|index| index * 20).collect();
        assert_eq!(
            evaluate_late(TestMode::Words10, &keys, &times),
            Err(Rejection::TooFast)
        );
    }

    #[test]
    fn rejects_machine_regular_rhythm() {
        let keys = perfect_keys(&targets(TestMode::Words10));
        let times: Vec<u32> = (0..keys.len() as u32).map(|index| index * 150).collect();
        assert_eq!(
            evaluate_late(TestMode::Words10, &keys, &times),
            Err(Rejection::InhumanRhythm)
        );
    }

    #[test]
    fn rejects_result_sent_before_test_could_end() {
        let keys = perfect_keys(&targets(TestMode::Time15));
        let keys: String = keys.chars().take(10).collect();
        let times = human_times(keys.len());
        let mode = TestMode::Time15;
        assert_eq!(
            evaluate(&ticket(mode), &keys, &times, ISSUED_AT_MS + 5_000).map(|scored| scored.stats),
            Err(Rejection::SubmittedTooEarly)
        );
    }

    #[test]
    fn rejects_expired_token() {
        let mode = TestMode::Time15;
        assert_eq!(
            evaluate(&ticket(mode), "a", &[0], ISSUED_AT_MS + TOKEN_TTL_MS + 1)
                .map(|scored| scored.stats),
            Err(Rejection::Expired)
        );
    }

    /// A wrong letter for `expected`, so a test never types the right key by accident.
    fn wrong_letter(expected: u8) -> char {
        if expected == b'z' { 'y' } else { 'z' }
    }

    fn breakdown_of(mode: TestMode, keys: &str) -> Breakdown {
        let times = human_times(keys.chars().count());
        score_late(mode, keys, &times)
            .expect("valid submission")
            .breakdown
    }

    fn key_stat(breakdown: &Breakdown, key: char) -> Option<KeyStat> {
        breakdown.keys.iter().copied().find(|stat| stat.key == key)
    }

    #[test]
    fn perfect_test_has_no_misses() {
        let words = targets(TestMode::Words10);
        let keys = perfect_keys(&words);

        let breakdown = breakdown_of(TestMode::Words10, &keys);

        assert!(breakdown.missed_words.is_empty());
        assert!(breakdown.keys.iter().all(|stat| stat.misses == 0));
        let hits: u32 = breakdown.keys.iter().map(|stat| stat.hits).sum();
        assert_eq!(hits as usize, keys.len());
        assert_eq!(
            key_stat(&breakdown, ' '),
            Some(KeyStat {
                key: ' ',
                hits: 9,
                misses: 0
            })
        );
    }

    #[test]
    fn corrected_typo_counts_as_missed_key_but_not_missed_word() {
        let words = targets(TestMode::Words10);
        let first = words[0].as_bytes()[0];
        let keys = format!("{}\u{8}{}", wrong_letter(first), perfect_keys(&words));

        let breakdown = breakdown_of(TestMode::Words10, &keys);

        assert!(breakdown.missed_words.is_empty());
        assert_eq!(breakdown.keys[0].key, char::from(first));
        assert_eq!(breakdown.keys[0].misses, 1);
    }

    #[test]
    fn word_left_wrong_is_reported() {
        let words = targets(TestMode::Words10);
        let first = words[0];
        let last = first.as_bytes()[first.len() - 1];
        let typo = format!("{}{}", &first[..first.len() - 1], wrong_letter(last));
        let keys = format!("{typo} {}", perfect_keys(&words[1..]));

        let breakdown = breakdown_of(TestMode::Words10, &keys);

        assert_eq!(
            breakdown.missed_words,
            [MissedWord {
                expected: first.to_owned(),
                typed: typo
            }]
        );
        assert_eq!(
            key_stat(&breakdown, char::from(last)).map(|stat| stat.misses),
            Some(1)
        );
        assert_eq!(
            key_stat(&breakdown, ' '),
            Some(KeyStat {
                key: ' ',
                hits: 8,
                misses: 1
            })
        );
    }

    #[test]
    fn premature_space_misses_the_skipped_letter() {
        let words = targets(TestMode::Words10);
        let first = words[0];
        let skipped = first.as_bytes()[first.len() - 1];
        let short = &first[..first.len() - 1];
        let keys = format!("{short} {}", perfect_keys(&words[1..]));

        let breakdown = breakdown_of(TestMode::Words10, &keys);

        assert_eq!(
            breakdown.missed_words,
            [MissedWord {
                expected: first.to_owned(),
                typed: short.to_owned()
            }]
        );
        assert_eq!(
            key_stat(&breakdown, char::from(skipped)).map(|stat| stat.misses),
            Some(1)
        );
        assert_eq!(key_stat(&breakdown, ' ').map(|stat| stat.misses), Some(0));
    }

    #[test]
    fn extra_letters_are_errors_without_an_expected_key() {
        let words = targets(TestMode::Words10);
        let keys = format!("{}q {}", words[0], perfect_keys(&words[1..]));
        let times = human_times(keys.len());

        let scored = score_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let misses: u32 = scored.breakdown.keys.iter().map(|stat| stat.misses).sum();
        assert_eq!(scored.stats.incorrect_keystrokes, 2);
        // The extra `q` has no expected key; only the space that commits the word is tallied.
        assert_eq!(misses, 1);
    }

    #[test]
    fn wrong_last_word_ends_test_without_credit() {
        let words = targets(TestMode::Words10);
        let last = words[9];
        let keys = format!("{} {} ", perfect_keys(&words[..9]), &last[..1]);
        let times = human_times(keys.len());

        let scored = score_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let elapsed_minutes = f64::from(*times.last().expect("non-empty")) / 60_000.0;
        let credited = perfect_keys(&words[..9]).len() + 1;
        assert_eq!(
            scored.stats.wpm,
            round2(credited as f64 / 5.0 / elapsed_minutes)
        );
        assert_eq!(
            scored.breakdown.missed_words,
            [MissedWord {
                expected: last.to_owned(),
                typed: last[..1].to_owned()
            }]
        );
    }

    #[test]
    fn unfinished_word_of_time_test_is_not_a_missed_word() {
        let words = targets(TestMode::Time15);
        let keys = format!("{} {}", words[0], &words[1][..1]);

        let breakdown = breakdown_of(TestMode::Time15, &keys);

        assert!(breakdown.missed_words.is_empty());
    }

    #[test]
    fn keys_sort_by_miss_rate_then_misses_then_key() {
        let stat = |key, hits, misses| KeyStat { key, hits, misses };
        let mut keys = [
            stat('a', 9, 1),
            stat('b', 1, 1),
            stat('c', 2, 2),
            stat('d', 5, 0),
            stat('e', 3, 1),
            stat('f', 3, 1),
        ];

        keys.sort_unstable_by(weaker_first);

        let order: String = keys.iter().map(|stat| stat.key).collect();
        assert_eq!(order, "cbefad");
    }

    /// Times that put `per_second[s]` keys inside second `s`, at uneven offsets so the
    /// rhythm check sees human-like gaps.
    fn times_per_second(per_second: &[u32]) -> Vec<u32> {
        const OFFSETS: [u32; 10] = [37, 121, 211, 298, 433, 517, 650, 744, 902, 980];
        per_second
            .iter()
            .enumerate()
            .flat_map(|(second, &count)| {
                OFFSETS[..count as usize]
                    .iter()
                    .map(move |offset| second as u32 * 1_000 + offset)
            })
            .collect()
    }

    #[test]
    fn raw_wpm_ignores_deleted_characters() {
        let words = targets(TestMode::Words10);
        let keys = format!("q\u{8}{}", perfect_keys(&words));
        let times = human_times(keys.chars().count());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        assert_eq!(stats.raw_wpm, stats.wpm);
        assert_eq!(
            (stats.correct_keystrokes + stats.incorrect_keystrokes) as usize,
            perfect_keys(&words).len() + 1
        );
    }

    #[test]
    fn raw_wpm_counts_space_that_ends_word_test() {
        let words = targets(TestMode::Words10);
        let keys = format!("{} {} ", perfect_keys(&words[..9]), &words[9][..1]);
        let times = human_times(keys.len());

        let stats = evaluate_late(TestMode::Words10, &keys, &times).expect("valid submission");

        let elapsed_minutes = f64::from(*times.last().expect("non-empty")) / 60_000.0;
        assert_eq!(
            stats.raw_wpm,
            round2(keys.len() as f64 / 5.0 / elapsed_minutes)
        );
    }

    #[test]
    fn steady_speed_every_second_is_fully_consistent() {
        let text = perfect_keys(&targets(TestMode::Time15));
        let times = times_per_second(&[5; 15]);
        let keys: String = text.chars().take(times.len()).collect();

        let stats = evaluate_late(TestMode::Time15, &keys, &times).expect("valid submission");

        assert_eq!(stats.consistency, 100.0);
    }

    #[test]
    fn alternating_speed_lowers_consistency() {
        let text = perfect_keys(&targets(TestMode::Time15));
        let per_second: Vec<u32> = (0..15)
            .map(|second| if second % 2 == 0 { 5 } else { 10 })
            .collect();
        let times = times_per_second(&per_second);
        let keys: String = text.chars().take(times.len()).collect();

        let stats = evaluate_late(TestMode::Time15, &keys, &times).expect("valid submission");

        assert_eq!(stats.consistency, 65.99);
    }

    /// Marks for `keys` at `times` with no errors and no WPM credit, as the replay records
    /// them, for testing [`timeline`] and [`consistency`] on their own.
    fn marks(keys: &str, times: &[u32]) -> Vec<SecondMark> {
        let mut marks: Vec<SecondMark> = Vec::new();
        for (key, &time_ms) in keys.chars().zip(times) {
            let second = second_of(time_ms);
            if marks.last().is_none_or(|mark| mark.second != second) {
                marks.push(SecondMark {
                    second,
                    ..SecondMark::default()
                });
            }
            if key != BACKSPACE {
                marks.last_mut().expect("a mark was just pushed").chars += 1;
            }
        }
        marks
    }

    fn consistency_of(keys: &str, times: &[u32], elapsed_ms: u32, partial: bool) -> f64 {
        consistency(&timeline(&marks(keys, times), 0, elapsed_ms, partial))
    }

    #[test]
    fn partial_last_second_counts_only_for_word_tests_from_half_a_second() {
        let keys = "a".repeat(12);
        let mut times = times_per_second(&[5, 5]);
        times.extend([2_100, 2_400]);

        assert_eq!(consistency_of(&keys, &times, 2_600, true), 82.32);
        assert_eq!(consistency_of(&keys, &times, 2_600, false), 100.0);
        assert_eq!(consistency_of(&keys, &times, 2_400, true), 100.0);
    }

    #[test]
    fn backspace_does_not_count_toward_consistency() {
        let keys = "aaaaa\u{8}\u{8}\u{8}\u{8}\u{8}aaaaa";
        let mut times = times_per_second(&[10]);
        times.extend(times_per_second(&[0, 5]));

        assert_eq!(consistency_of(keys, &times, 2_000, false), 100.0);
    }

    #[test]
    fn consistency_is_zero_without_a_full_second() {
        assert_eq!(consistency_of("abc", &[0, 200, 400], 900, false), 0.0);
    }

    #[test]
    fn timeline_has_a_point_per_second_ending_at_the_test_wpm() {
        let keys = perfect_keys(&targets(TestMode::Words10));
        let mut times = human_times(keys.len());
        // End the test on a whole second, so its last second is fully scored.
        let last = times.len() - 1;
        times[last] = (times[last - 1] / 1_000 + 1) * 1_000;

        let scored = score_late(TestMode::Words10, &keys, &times).expect("valid submission");

        assert_eq!(
            scored.timeline.len(),
            (scored.stats.elapsed_ms / 1_000) as usize
        );
        let last_point = scored
            .timeline
            .last()
            .expect("the test lasts over a second");
        assert_eq!(last_point.wpm, scored.stats.wpm);
        assert!(scored.timeline.iter().all(|point| point.errors == 0));
    }

    #[test]
    fn timeline_places_errors_in_their_second() {
        let words = targets(TestMode::Time15);
        // Second 0 types the first word right; second 2 types a wrong letter; second 1 is idle.
        let first = words[0];
        let mut keys = format!("{first} ");
        let mut times: Vec<u32> = (0..keys.len() as u32).map(|i| 100 + i * 50).collect();
        let wrong = if words[1].starts_with('x') { 'z' } else { 'x' };
        keys.push(wrong);
        times.push(2_500);

        let scored = score_late(TestMode::Time15, &keys, &times).expect("valid submission");

        let timeline = &scored.timeline;
        assert_eq!(timeline.len(), 15);
        assert_eq!(timeline[0].errors, 0);
        assert_eq!(timeline[2].errors, 1);
        assert_eq!(timeline.iter().map(|p| p.errors).sum::<u32>(), 1);
        assert_eq!(timeline[1].raw_wpm, 0.0);
        assert_eq!(timeline[2].raw_wpm, 12.0);
        // The first word and its space stay credited while the clock runs on.
        let credited = f64::from(first.len() as u32 + 1);
        assert_eq!(timeline[1].wpm, round2(credited * 12.0 / 2.0));
        assert_eq!(timeline[14].wpm, scored.stats.wpm);
    }

    #[test]
    fn ignored_spaces_do_not_count_toward_raw_speed() {
        let word = targets(TestMode::Time15)[0];
        // The first space commits the word; the four after it come before any letter of the
        // next word, so the replay ignores them.
        let keys = format!("{word}     ");
        let times: Vec<u32> = (0..keys.len() as u32)
            .map(|index| 40 + index * 61 + index % 3 * 17)
            .collect();

        let scored = score_late(TestMode::Time15, &keys, &times).expect("valid submission");

        let counted = word.len() as u32 + 1;
        assert_eq!(
            scored.stats.correct_keystrokes + scored.stats.incorrect_keystrokes,
            counted
        );
        assert_eq!(scored.timeline[0].raw_wpm, f64::from(counted * 12));
    }

    #[test]
    fn last_point_includes_keys_of_an_unscored_partial_second() {
        let keys = perfect_keys(&targets(TestMode::Words10));
        let mut times = human_times(keys.len());
        // End the test 300 ms into a second, too short for that second to get its own point.
        let last = times.len() - 1;
        times[last] = (times[last - 1] / 1_000 + 1) * 1_000 + 300;

        let scored = score_late(TestMode::Words10, &keys, &times).expect("valid submission");

        assert_eq!(
            scored.timeline.len(),
            (scored.stats.elapsed_ms / 1_000) as usize
        );
        let last_point = scored
            .timeline
            .last()
            .expect("the test lasts over a second");
        assert_eq!(last_point.wpm, scored.stats.wpm);
    }

    #[test]
    fn rejection_codes_are_snake_case() {
        let json = serde_json::to_string(&Rejection::InhumanRhythm).expect("serializes");
        assert_eq!(json, r#""inhuman_rhythm""#);
    }
}
