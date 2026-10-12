//! Server-side scoring of a keystroke log.
//!
//! The client never reports its own WPM. It sends every keystroke with a timestamp, and the
//! server replays them against the text it regenerates from the token. Scoring follows the
//! common typing-test rules: space moves to the next word, backspace may return to the
//! previous word, and WPM counts the characters of correctly typed words (plus their spaces)
//! divided by five per minute.
//!
//! Everything here is pure: it takes the clock as an argument, so it is fully unit-testable.

use serde::Serialize;
use thiserror::Error;

use crate::mode::{MAX_RAW_WPM, TestMode};
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
    /// Words per minute from every character typed, right or wrong.
    pub raw_wpm: f64,
    /// Percentage of character keystrokes that were correct when typed.
    pub accuracy: f64,
    /// Rhythm steadiness from 0 to 100, from the variation between key intervals.
    pub consistency: f64,
    /// Character keystrokes that matched the text when typed.
    pub correct_keystrokes: u32,
    /// Character keystrokes that did not match the text when typed.
    pub incorrect_keystrokes: u32,
    /// Duration the scores are computed over, in milliseconds.
    pub elapsed_ms: u32,
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
    /// Raw speed above [`MAX_RAW_WPM`].
    #[error("the typing speed is above the {MAX_RAW_WPM} WPM limit")]
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
/// The verified [`TestStats`].
///
/// # Errors
///
/// Returns a [`Rejection`] that names the first rule the submission broke.
///
/// # Examples
///
/// ```ignore
/// let stats = evaluate(&ticket, "the be", &[0, 150, 300, 450, 600, 750], now_ms)?;
/// assert!(stats.wpm > 0.0);
/// ```
pub fn evaluate(
    ticket: &TestTicket,
    keys: &str,
    times_ms: &[u32],
    now_ms: u64,
) -> Result<TestStats, Rejection> {
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
    replay.into_stats()
}

/// Running mean and variance of key intervals (Welford's one-pass algorithm).
#[derive(Debug, Default)]
struct IntervalStats {
    count: u32,
    mean: f64,
    sum_squared_deviation: f64,
}

impl IntervalStats {
    fn push(&mut self, interval_ms: u32) {
        self.count += 1;
        let value = f64::from(interval_ms);
        let delta = value - self.mean;
        self.mean += delta / f64::from(self.count);
        self.sum_squared_deviation += delta * (value - self.mean);
    }

    /// Standard deviation divided by mean. `None` with fewer than two samples.
    fn coefficient_of_variation(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }
        if self.mean == 0.0 {
            return Some(0.0);
        }
        Some((self.sum_squared_deviation / f64::from(self.count)).sqrt() / self.mean)
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
    correct_keystrokes: u32,
    incorrect_keystrokes: u32,
    intervals: IntervalStats,
    elapsed_ms: u32,
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
            correct_keystrokes: 0,
            incorrect_keystrokes: 0,
            intervals: IntervalStats::default(),
            elapsed_ms: 0,
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
                replay.intervals.push(time_ms - previous_ms);
            }
            previous_ms = time_ms;

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
            }
        }

        match time_limit_ms {
            Some(limit_ms) => replay.elapsed_ms = limit_ms,
            None if !finished => return Err(Rejection::Incomplete),
            None => {}
        }
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

    fn record(&mut self, correct: bool) {
        if correct {
            self.correct_keystrokes += 1;
        } else {
            self.incorrect_keystrokes += 1;
        }
    }

    /// Types one character. Returns `true` when it completes the last word correctly.
    fn character(&mut self, character: char) -> bool {
        let expected = self.target().as_bytes().get(self.word_chars).copied();
        self.record(expected.is_some_and(|byte| char::from(byte) == character));
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
        self.record(self.current_word() == self.target());
        if self.is_last_word() {
            return true;
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

    /// Characters that count toward WPM: each correctly committed word plus its space, and
    /// the correct prefix of the word in progress.
    fn correct_chars(&self) -> usize {
        let committed: usize = self.typed[..self.word_start]
            .split_terminator(' ')
            .zip(self.targets)
            .filter(|(typed, target)| typed == *target)
            .map(|(typed, _)| typed.len() + 1)
            .sum();
        let current = self.current_word();
        let in_progress = if self.target().starts_with(current) {
            current.len()
        } else {
            0
        };
        committed + in_progress
    }

    fn into_stats(self) -> Result<TestStats, Rejection> {
        let typed_keystrokes = self.correct_keystrokes + self.incorrect_keystrokes;
        if typed_keystrokes == 0 {
            return Err(Rejection::Empty);
        }
        if self.elapsed_ms == 0 {
            return Err(Rejection::TooFast);
        }
        let minutes = f64::from(self.elapsed_ms) / 60_000.0;
        let words_per_minute = |chars: f64| chars / 5.0 / minutes;
        let raw_wpm = words_per_minute(f64::from(typed_keystrokes));
        if raw_wpm > f64::from(MAX_RAW_WPM) {
            return Err(Rejection::TooFast);
        }
        let interval_cv = self.intervals.coefficient_of_variation();
        if self.intervals.count >= MIN_RHYTHM_SAMPLES
            && interval_cv.is_some_and(|cv| cv < MIN_HUMAN_INTERVAL_CV)
        {
            return Err(Rejection::InhumanRhythm);
        }

        Ok(TestStats {
            wpm: round2(words_per_minute(self.correct_chars() as f64)),
            raw_wpm: round2(raw_wpm),
            accuracy: round2(
                f64::from(self.correct_keystrokes) * 100.0 / f64::from(typed_keystrokes),
            ),
            consistency: round2(interval_cv.map_or(100.0, |cv| (100.0 * (1.0 - cv)).max(0.0))),
            correct_keystrokes: self.correct_keystrokes,
            incorrect_keystrokes: self.incorrect_keystrokes,
            elapsed_ms: self.elapsed_ms,
        })
    }
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

    fn evaluate_late(mode: TestMode, keys: &str, times: &[u32]) -> Result<TestStats, Rejection> {
        evaluate(&ticket(mode), keys, times, ISSUED_AT_MS + 10 * 60 * 1_000)
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
            evaluate(&ticket(mode), &keys, &times, ISSUED_AT_MS + 5_000),
            Err(Rejection::SubmittedTooEarly)
        );
    }

    #[test]
    fn rejects_expired_token() {
        let mode = TestMode::Time15;
        assert_eq!(
            evaluate(&ticket(mode), "a", &[0], ISSUED_AT_MS + TOKEN_TTL_MS + 1),
            Err(Rejection::Expired)
        );
    }

    #[test]
    fn rejection_codes_are_snake_case() {
        let json = serde_json::to_string(&Rejection::InhumanRhythm).expect("serializes");
        assert_eq!(json, r#""inhuman_rhythm""#);
    }
}
