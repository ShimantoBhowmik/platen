//! Test modes the server offers.
//!
//! The wire format is a loose `{ "mode": "time", "amount": 30 }` pair ([`ModeSpec`]). It is
//! parsed once at the HTTP boundary into [`TestMode`], a closed enum, so an unsupported
//! combination such as "time 17s" cannot exist anywhere inside the server.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Highest raw speed a submission may reach. It also sizes the word list for time tests, so a
/// typist under this speed never runs out of words.
pub const MAX_RAW_WPM: u32 = 350;

/// Fewest keystrokes that can finish a word: one letter and a space.
const MIN_KEYSTROKES_PER_WORD: u32 = 2;

/// A supported test configuration.
///
/// The discriminants are a stable compact code stored in tokens and in the database, so the
/// order of variants must never change. New variants go at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TestMode {
    /// 15 second time test.
    Time15 = 0,
    /// 30 second time test.
    Time30 = 1,
    /// 60 second time test.
    Time60 = 2,
    /// 120 second time test.
    Time120 = 3,
    /// 10 word test.
    Words10 = 4,
    /// 25 word test.
    Words25 = 5,
    /// 50 word test.
    Words50 = 6,
    /// 100 word test.
    Words100 = 7,
}

/// What ends a test: a fixed duration or a fixed number of words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeKind {
    /// Ends after a fixed number of seconds.
    Time,
    /// Ends after a fixed number of words.
    Words,
}

/// Wire representation of a mode, e.g. `{ "mode": "words", "amount": 25 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSpec {
    /// Whether the test is timed or has a word count.
    pub mode: ModeKind,
    /// Seconds for [`ModeKind::Time`], words for [`ModeKind::Words`].
    pub amount: u16,
}

/// Returned when a [`ModeSpec`] names a combination the server does not offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported mode {} {}", .0.mode.as_str(), .0.amount)]
pub struct UnsupportedMode(pub ModeSpec);

impl ModeKind {
    /// The wire name, as used in JSON and query strings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Words => "words",
        }
    }
}

impl TestMode {
    /// Every mode, indexed by its code.
    pub const ALL: [Self; 8] = [
        Self::Time15,
        Self::Time30,
        Self::Time60,
        Self::Time120,
        Self::Words10,
        Self::Words25,
        Self::Words50,
        Self::Words100,
    ];

    /// Compact code used in tokens and the database.
    pub const fn code(self) -> u8 {
        // `as` on a fieldless `#[repr(u8)]` enum reads its discriminant.
        self as u8
    }

    /// Inverse of [`TestMode::code`].
    ///
    /// # Returns
    ///
    /// `None` when `code` does not name a mode.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }

    /// The wire representation of this mode.
    pub const fn spec(self) -> ModeSpec {
        let (mode, amount) = match self {
            Self::Time15 => (ModeKind::Time, 15),
            Self::Time30 => (ModeKind::Time, 30),
            Self::Time60 => (ModeKind::Time, 60),
            Self::Time120 => (ModeKind::Time, 120),
            Self::Words10 => (ModeKind::Words, 10),
            Self::Words25 => (ModeKind::Words, 25),
            Self::Words50 => (ModeKind::Words, 50),
            Self::Words100 => (ModeKind::Words, 100),
        };
        ModeSpec { mode, amount }
    }

    /// Test duration in milliseconds, or `None` for word-count tests.
    pub const fn time_limit_ms(self) -> Option<u32> {
        let spec = self.spec();
        match spec.mode {
            ModeKind::Time => Some(spec.amount as u32 * 1_000),
            ModeKind::Words => None,
        }
    }

    /// How many words the test text contains.
    ///
    /// For time tests this is enough words that even one-letter words typed at [`MAX_RAW_WPM`]
    /// cannot run out. A raw WPM "word" is 5 keystrokes, while a typed word can cost as few as
    /// [`MIN_KEYSTROKES_PER_WORD`].
    pub const fn word_count(self) -> usize {
        let spec = self.spec();
        match spec.mode {
            ModeKind::Time => (spec.amount as u32 * MAX_RAW_WPM * 5)
                .div_ceil(60 * MIN_KEYSTROKES_PER_WORD) as usize,
            ModeKind::Words => spec.amount as usize,
        }
    }
}

impl TryFrom<ModeSpec> for TestMode {
    type Error = UnsupportedMode;

    /// Parses a wire mode, rejecting combinations the server does not offer.
    ///
    /// # Errors
    ///
    /// Returns [`UnsupportedMode`] when no [`TestMode`] has this spec.
    fn try_from(spec: ModeSpec) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.spec() == spec)
            .ok_or(UnsupportedMode(spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_for_every_mode() {
        for mode in TestMode::ALL {
            assert_eq!(TestMode::from_code(mode.code()), Some(mode));
        }
        assert_eq!(TestMode::from_code(8), None);
    }

    #[test]
    fn specs_round_trip_for_every_mode() {
        for mode in TestMode::ALL {
            assert_eq!(TestMode::try_from(mode.spec()), Ok(mode));
        }
    }

    #[test]
    fn rejects_unsupported_spec() {
        let spec = ModeSpec {
            mode: ModeKind::Time,
            amount: 17,
        };
        assert_eq!(TestMode::try_from(spec), Err(UnsupportedMode(spec)));
        assert_eq!(
            UnsupportedMode(spec).to_string(),
            "unsupported mode time 17"
        );
    }

    #[test]
    fn time_mode_has_limit_and_enough_words() {
        assert_eq!(TestMode::Time60.time_limit_ms(), Some(60_000));
        assert_eq!(TestMode::Time60.word_count(), 875);
        assert_eq!(TestMode::Time15.word_count(), 219);
    }

    #[test]
    fn words_mode_has_no_limit() {
        assert_eq!(TestMode::Words25.time_limit_ms(), None);
        assert_eq!(TestMode::Words25.word_count(), 25);
    }

    #[test]
    fn spec_uses_lowercase_wire_names() {
        let json = serde_json::to_string(&TestMode::Words50.spec()).expect("spec serializes");
        assert_eq!(json, r#"{"mode":"words","amount":50}"#);
    }
}
