/-!
# Per-second marks and the scored duration (`server/src/scoring.rs`)

The record `Replay::run` keeps for each second that has keys, and that `timeline` turns into
the speed chart, with the two time helpers both use. Shared by `Platen.Replay` and
`Platen.Timeline`.
-/

namespace Platen

/-- `SecondMark`. -/
structure SecondMark where
  /-- Zero-based second of the test, from `secondOf`. -/
  second : Nat
  /-- Character keys counted in this second. -/
  chars : Nat
  /-- Character keys in this second that did not match the text. -/
  errors : Nat
  /-- Characters that counted toward WPM when this second's first key arrived. -/
  correctCharsBefore : Nat
  deriving DecidableEq, Repr

/-- `second_of`: a key at exactly 1,000 ms closes the first second. -/
def secondOf (timeMs : Nat) : Nat := (timeMs - 1) / 1000

theorem secondOf_mono {a b : Nat} (h : a ≤ b) : secondOf a ≤ secondOf b := by
  unfold secondOf; exact Nat.div_le_div_right (by omega)

/-- `scored_ms`: the duration the scores are computed over, the test's length rounded to the
nearest 10 ms, halves up. -/
def scoredMs (elapsedMs : Nat) : Nat := (elapsedMs + 5) / 10 * 10

/-- The rounding lands on a multiple of 10 at most 5 ms away. -/
theorem scoredMs_near (e : Nat) : e ≤ scoredMs e + 5 ∧ scoredMs e ≤ e + 5 ∧ scoredMs e % 10 = 0 := by
  unfold scoredMs; omega

end Platen
