import Platen.Marks

/-!
# Speed timeline (`server/src/scoring.rs`)

`timeline` expands the replay's sparse per-second marks into one point per scored second.

This models `timeline` with its last-point fix. The unfixed code gives the last point the
`correct_chars_before` of the next mark and ends it at the last whole second, so a word test
whose unscored partial second holds keys ends its timeline off the test's WPM: at 2,300 ms
with a key in second 2, the last point counts the chars before that key over 2 s.

Abstractions: `Nat` for the `u32` counts and milliseconds. A point keeps the integer pairs its
two floats are computed from instead of the floats: `wpm` is `correct_chars * 12 / through_s`
and `raw_wpm` is `chars * 12 / span_s`, so equal pairs give equal floats. `whole_seconds +
u32::from(has_tail)` cannot overflow because the caller bounds `elapsed_ms` first. The mutable
cursor `next` is modelled by the suffix `marks[next..]` it points into.
-/

namespace Platen

/-- `TimelinePoint`, as the integers its floats are computed from. -/
structure TimelinePoint where
  /-- Characters `wpm` counts. -/
  correctChars : Nat
  /-- Milliseconds from the start of the test that `wpm` counts them over. -/
  throughMs : Nat
  /-- Character keys `raw_wpm` counts. -/
  chars : Nat
  /-- Milliseconds `raw_wpm` counts them over. -/
  spanMs : Nat
  errors : Nat
  deriving DecidableEq, Repr, Inhabited

/-- The marks ascend strictly by second, as `Replay::run` pushes them. -/
def Ascending (marks : List SecondMark) : Prop :=
  marks.Pairwise fun a b => a.second < b.second

/-- `has_tail`: a word test scores a last partial second of at least 500 ms. -/
def hasTail (elapsedMs : Nat) (scoresPartialSecond : Bool) : Bool :=
  scoresPartialSecond && decide (500 ≤ elapsedMs % 1000)

/-- `whole_seconds + u32::from(has_tail)`: how many points the timeline has. -/
def pointCount (elapsedMs : Nat) (scoresPartialSecond : Bool) : Nat :=
  elapsedMs / 1000 + if hasTail elapsedMs scoresPartialSecond then 1 else 0

/-- `marks.get(next).filter(|mark| mark.second == second)`, with the cursor after it: the mark
of `second` if the cursor is on it, and the marks still to pair. -/
def consume (rest : List SecondMark) (second : Nat) : Option SecondMark × List SecondMark :=
  match rest with
  | m :: ms => if m.second = second then (some m, ms) else (none, rest)
  | [] => (none, [])

/-- The closure `timeline` maps over `0..count`: the points from `second` on, with `rest` the
marks from `next` on and `n` points left. The last point counts `final_correct_chars` through
`elapsed_ms`, so it equals the test's WPM even when an unscored partial second holds keys;
every other point counts what the next second with keys starts from, through its own end. -/
def timelineFrom (finalCorrectChars elapsedMs count : Nat) :
    List SecondMark → Nat → Nat → List TimelinePoint
  | _, _, 0 => []
  | rest, second, n + 1 =>
    let wholeSeconds := elapsedMs / 1000
    let tailMs := elapsedMs % 1000
    let (here, rest') := consume rest second
    let last := second + 1 = count
    let correctChars :=
      if last then finalCorrectChars
      else match rest' with
        | m :: _ => m.correctCharsBefore
        | [] => finalCorrectChars
    let spanMs := if second < wholeSeconds then 1000 else tailMs
    let throughMs := if last then elapsedMs else (second + 1) * 1000
    { correctChars, throughMs, chars := (here.map (·.chars)).getD 0, spanMs,
      errors := (here.map (·.errors)).getD 0 } ::
      timelineFrom finalCorrectChars elapsedMs count rest' (second + 1) n

/-- `timeline`. -/
def timeline (marks : List SecondMark) (finalCorrectChars elapsedMs : Nat)
    (scoresPartialSecond : Bool) : List TimelinePoint :=
  let count := pointCount elapsedMs scoresPartialSecond
  timelineFrom finalCorrectChars elapsedMs count marks 0 count

/-- The mark of `second`, if that second has keys. -/
def markAt (marks : List SecondMark) (second : Nat) : Option SecondMark :=
  marks.find? (·.second = second)

theorem length_timelineFrom (f e c : Nat) (rest : List SecondMark) (second n : Nat) :
    (timelineFrom f e c rest second n).length = n := by
  induction n generalizing rest second with
  | zero => rfl
  | succ n ih => simp [timelineFrom, ih]

/-- One point per whole second, plus one for a scored partial second of at least 500 ms. -/
theorem length_timeline (marks : List SecondMark) (f elapsedMs : Nat) (p : Bool) :
    (timeline marks f elapsedMs p).length =
      elapsedMs / 1000 + if p ∧ 500 ≤ elapsedMs % 1000 then 1 else 0 := by
  simp [timeline, length_timelineFrom, pointCount, hasTail]

theorem markAt_none {rest : List SecondMark} {second : Nat}
    (h : ∀ m ∈ rest, second < m.second) : markAt rest second = none := by
  rw [markAt, List.find?_eq_none]
  intro m hm; have := h m hm; simp; omega

/-- What `consume` pairs: the mark of `second` when the cursor's marks all come at or after it,
and the marks left all come after it. -/
theorem consume_spec {rest : List SecondMark} {second : Nat} (hasc : Ascending rest)
    (hge : ∀ m ∈ rest, second ≤ m.second) :
    (consume rest second).1 = markAt rest second ∧ Ascending (consume rest second).2 ∧
      (∀ m ∈ (consume rest second).2, second + 1 ≤ m.second) ∧
      ∀ s, second < s → markAt (consume rest second).2 s = markAt rest s := by
  unfold Ascending at hasc
  cases rest with
  | nil => simp [consume, markAt, Ascending]
  | cons m ms =>
    rw [List.pairwise_cons] at hasc
    have hm := hge m (List.mem_cons_self ..)
    by_cases hs : m.second = second
    · simp only [consume, hs, ite_true]
      refine ⟨by simp [markAt, hs], hasc.2, fun x hx => ?_, fun s hlt => ?_⟩
      · have := hasc.1 x hx; omega
      · simp [markAt, hs, show second ≠ s by omega]
    · simp only [consume, hs, ite_false]
      refine ⟨(markAt_none fun x hx => ?_).symm, List.pairwise_cons.mpr hasc, fun x hx => ?_,
        fun _ _ => trivial⟩
      · rcases List.mem_cons.mp hx with rfl | hx
        · omega
        · have := hasc.1 x hx; omega
      · rcases List.mem_cons.mp hx with rfl | hx
        · omega
        · have := hasc.1 x hx; omega

/-- Point `i` from `second` on has the chars and errors of the mark of second `second + i`. -/
theorem timelineFrom_pairs (f e c : Nat) (n : Nat) :
    ∀ (rest : List SecondMark) (second i : Nat), Ascending rest →
      (∀ m ∈ rest, second ≤ m.second) → i < n → ∀ pt,
      (timelineFrom f e c rest second n)[i]? = some pt →
        pt.chars = ((markAt rest (second + i)).map (·.chars)).getD 0 ∧
        pt.errors = ((markAt rest (second + i)).map (·.errors)).getD 0 := by
  induction n with
  | zero => intro _ _ _ _ _ h; omega
  | succ n ih =>
    intro rest second i hasc hge h pt hpt
    obtain ⟨h1, h2, h3, h4⟩ := consume_spec hasc hge
    cases i with
    | zero => simp [timelineFrom, h1] at hpt; subst hpt; simp
    | succ i =>
      simp only [timelineFrom, List.getElem?_cons_succ] at hpt
      have := ih _ (second + 1) i h2 h3 (by omega) pt hpt
      rwa [h4 (second + 1 + i) (by omega), Nat.add_assoc, Nat.add_comm 1 i] at this

/-- Point `s` has the chars and errors of the mark whose `second` is `s`, or 0 without one. -/
theorem timeline_pairs {marks : List SecondMark} (hasc : Ascending marks) (f elapsedMs : Nat)
    (p : Bool) (s : Nat) (h : s < (timeline marks f elapsedMs p).length) :
    (timeline marks f elapsedMs p)[s].chars = ((markAt marks s).map (·.chars)).getD 0 ∧
      (timeline marks f elapsedMs p)[s].errors = ((markAt marks s).map (·.errors)).getD 0 := by
  have hn : s < pointCount elapsedMs p := by simpa [timeline, length_timelineFrom] using h
  simpa using timelineFrom_pairs f elapsedMs _ _ marks 0 s hasc (fun _ _ => Nat.zero_le _) hn _
    (List.getElem?_eq_getElem h)

/-- Sum of the errors of some marks. -/
def errorSum (marks : List SecondMark) : Nat := (marks.map (·.errors)).sum

theorem timelineFrom_errors (f e c : Nat) (n : Nat) :
    ∀ (rest : List SecondMark) (second : Nat), Ascending rest →
      (∀ m ∈ rest, second ≤ m.second) →
      ((timelineFrom f e c rest second n).map (·.errors)).sum =
        errorSum (rest.filter (·.second < second + n)) := by
  induction n with
  | zero =>
    intro rest second _ hge
    rw [List.filter_eq_nil_iff.mpr (fun m hm => by have := hge m hm; simp; omega)]
    rfl
  | succ n ih =>
    intro rest second hasc hge
    obtain ⟨-, h2, h3, -⟩ := consume_spec hasc hge
    simp only [timelineFrom, List.map_cons, List.sum_cons]
    rw [ih _ (second + 1) h2 h3, show second + 1 + n = second + (n + 1) by omega]
    cases rest with
    | nil => simp [consume, errorSum]
    | cons m ms => by_cases hs : m.second = second <;> simp [consume, hs, errorSum]

/-- The errors over the points add up to the errors of the marks in the scored seconds. -/
theorem timeline_errors {marks : List SecondMark} (hasc : Ascending marks) (f elapsedMs : Nat)
    (p : Bool) :
    ((timeline marks f elapsedMs p).map (·.errors)).sum =
      errorSum (marks.filter (·.second < pointCount elapsedMs p)) := by
  simpa [timeline] using
    timelineFrom_errors f elapsedMs _ _ marks 0 hasc (fun _ _ => Nat.zero_le _)

theorem timelineFrom_last (f e c : Nat) (n : Nat) :
    ∀ (rest : List SecondMark) (second : Nat), second + n = c →
      ∀ pt, (timelineFrom f e c rest second n).getLast? = some pt →
        pt.correctChars = f ∧ pt.throughMs = e := by
  induction n with
  | zero => intro _ _ _ _ h; simp [timelineFrom] at h
  | succ n ih =>
    intro rest second hc pt h
    cases n with
    | zero =>
      have hl : second + 1 = c := hc
      simp [timelineFrom, hl] at h
      subst h; simp
    | succ n =>
      simp only [timelineFrom] at h
      rw [List.getLast?_cons_cons] at h
      exact ih _ (second + 1) (by omega) pt h

/-- The last point equals the test's WPM: it counts `final_correct_chars` through
`elapsed_ms`. -/
theorem timeline_last (marks : List SecondMark) (f elapsedMs : Nat) (p : Bool)
    {pt : TimelinePoint} (h : (timeline marks f elapsedMs p).getLast? = some pt) :
    pt.correctChars = f ∧ pt.throughMs = elapsedMs :=
  timelineFrom_last f elapsedMs _ _ marks 0 (Nat.zero_add _) pt h

private def marksOf (seconds : List (Nat × Nat × Nat × Nat)) : List SecondMark :=
  seconds.map fun (second, chars, errors, before) => ⟨second, chars, errors, before⟩

-- `timeline_places_errors_in_their_second`: a 15 s test, a word of 4 letters and its space in
-- second 0, nothing in second 1, one wrong key in second 2.
#guard
  let t := timeline (marksOf [(0, 5, 0, 0), (2, 1, 1, 5)]) 5 15000 false
  t.length = 15 && t[0]!.errors = 0 && t[2]!.errors = 1 && (t.map (·.errors)).sum = 1 &&
    t[1]!.chars = 0 && t[2]!.chars = 1 && t[2]!.spanMs = 1000 &&
    t[1]!.correctChars = 5 && t[1]!.throughMs = 2000 &&
    t[14]!.correctChars = 5 && t[14]!.throughMs = 15000

-- `partial_last_second_counts_only_for_word_tests_from_half_a_second`: a scored 600 ms tail
-- gets its own point over 600 ms; a 400 ms tail does not.
#guard (timeline (marksOf [(0, 5, 0, 0), (1, 5, 0, 0), (2, 2, 0, 0)]) 0 2600 true).length = 3
#guard (timeline (marksOf [(0, 5, 0, 0), (1, 5, 0, 0), (2, 2, 0, 0)]) 0 2600 true)[2]!.spanMs = 600
#guard (timeline (marksOf [(0, 5, 0, 0), (1, 5, 0, 0), (2, 2, 0, 0)]) 0 2600 false).length = 2
#guard (timeline (marksOf [(0, 5, 0, 0), (1, 5, 0, 0), (2, 2, 0, 0)]) 0 2400 true).length = 2

-- A word test ending 300 ms into an unscored second that holds keys: the last point still
-- counts every credited character through the whole 2.3 s.
#guard (timeline (marksOf [(0, 5, 0, 0), (1, 5, 0, 5), (2, 2, 0, 10)]) 12 2300 true).getLast? =
  some ⟨12, 2300, 5, 1000, 0⟩

end Platen
