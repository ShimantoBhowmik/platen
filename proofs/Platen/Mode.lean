/-!
# Test modes (`server/src/mode.rs`)

The closed set of test configurations, their compact codes, and how many words a test's text
holds. The central result is `time_text_outlasts_cap`: a typist under the speed cap can never
reach the end of a time test's text, which is what lets `Replay::run` reject reaching it as
`TooFast`.
-/

namespace Platen

inductive TestMode
  | time15 | time30 | time60 | time120 | words10 | words25 | words50 | words100
  deriving DecidableEq, Repr

inductive ModeKind
  | time | words
  deriving DecidableEq, Repr

structure ModeSpec where
  mode : ModeKind
  amount : Nat
  deriving DecidableEq, Repr

/-- `MAX_KEYSTROKE_WPM`. -/
def maxKeystrokeWpm : Nat := 350

/-- `MIN_KEYSTROKES_PER_WORD`: one letter and a space. -/
def minKeystrokesPerWord : Nat := 2

namespace TestMode

/-- `TestMode::ALL`, indexed by code. -/
def all : List TestMode :=
  [time15, time30, time60, time120, words10, words25, words50, words100]

/-- `TestMode::code`, the `#[repr(u8)]` discriminant. -/
def code : TestMode → Nat
  | time15 => 0 | time30 => 1 | time60 => 2 | time120 => 3
  | words10 => 4 | words25 => 5 | words50 => 6 | words100 => 7

/-- `TestMode::from_code`. -/
def fromCode (code : Nat) : Option TestMode := all[code]?

/-- `TestMode::spec`. -/
def spec : TestMode → ModeSpec
  | time15 => ⟨.time, 15⟩ | time30 => ⟨.time, 30⟩
  | time60 => ⟨.time, 60⟩ | time120 => ⟨.time, 120⟩
  | words10 => ⟨.words, 10⟩ | words25 => ⟨.words, 25⟩
  | words50 => ⟨.words, 50⟩ | words100 => ⟨.words, 100⟩

/-- `TryFrom<ModeSpec> for TestMode`. -/
def tryFrom (s : ModeSpec) : Option TestMode := all.find? (·.spec == s)

/-- `TestMode::time_limit_ms`. -/
def timeLimitMs (m : TestMode) : Option Nat :=
  match m.spec.mode with
  | .time => some (m.spec.amount * 1000)
  | .words => none

/-- `TestMode::word_count`. -/
def wordCount (m : TestMode) : Nat :=
  match m.spec.mode with
  | .time =>
    let budget := m.spec.amount * maxKeystrokeWpm * 5 / 60
    (budget + 1) / minKeystrokesPerWord + 1
  | .words => m.spec.amount

theorem fromCode_code (m : TestMode) : fromCode m.code = some m := by
  cases m <;> rfl

theorem code_lt (m : TestMode) : m.code < 8 := by
  cases m <;> decide

theorem fromCode_none {c : Nat} (h : 8 ≤ c) : fromCode c = none := by
  simp [fromCode, all]; omega

theorem tryFrom_spec (m : TestMode) : tryFrom m.spec = some m := by
  cases m <;> rfl

theorem tryFrom_sound {s : ModeSpec} {m : TestMode} (h : tryFrom s = some m) : m.spec = s := by
  have := List.find?_some h
  simpa using this

theorem wordCount_pos (m : TestMode) : 0 < m.wordCount := by
  cases m <;> decide

end TestMode

/-- Whether `keystrokes` character keys typed over `limitMs` stay under the speed cap.

`Replay::into_scored` rejects `keystrokes / 5 / (ms / 60_000) > MAX_KEYSTROKE_WPM`, which over
the naturals is `keystrokes * 12_000 > MAX_KEYSTROKE_WPM * ms`. -/
def underCap (keystrokes limitMs : Nat) : Prop :=
  keystrokes * 12000 ≤ maxKeystrokeWpm * limitMs

instance (k ms : Nat) : Decidable (underCap k ms) := by
  unfold underCap; infer_instance

/-- The fewest character keys that finish `n` words: one letter each, and a space between
words but none after the last. `Replay.charKeys_ge_of_finished` proves the replay needs this
many. -/
def minKeystrokesToFinish (n : Nat) : Nat := minKeystrokesPerWord * n - 1

/-- A typist under the cap never reaches the end of a time test's text.

This is the claim behind `Replay::run` answering `TooFast` when a time test finishes, and
behind the doc of `TestMode::word_count`. -/
theorem time_text_outlasts_cap (m : TestMode) {limit : Nat} (hm : m.timeLimitMs = some limit)
    {k : Nat} (hk : underCap k limit) : k < minKeystrokesToFinish m.wordCount := by
  cases m <;> simp [TestMode.timeLimitMs, TestMode.spec] at hm <;> subst hm <;>
    simp [underCap, maxKeystrokeWpm, minKeystrokesToFinish, minKeystrokesPerWord,
      TestMode.wordCount, TestMode.spec] at hk ⊢ <;> omega

/-- The text is no longer than it must be: one word fewer could be finished under the cap. -/
theorem time_wordCount_minimal (m : TestMode) {limit : Nat} (hm : m.timeLimitMs = some limit) :
    underCap (minKeystrokesToFinish (m.wordCount - 1)) limit := by
  cases m <;> simp [TestMode.timeLimitMs, TestMode.spec] at hm <;> subst hm <;> decide

end Platen
