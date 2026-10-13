import Platen.Mode
import Platen.Marks

/-!
# The keystroke replay (`server/src/scoring.rs`, `Replay`)

The server replays a keystroke log against the test's words. This module models the integer
core of that replay: the typed buffer and its word bookkeeping (`character`, `space`,
`backspace`), the keystroke tallies (`record`), the per-second marks, the `run` loop with its
rejections, and the integer checks of `into_scored`.

Left out: the rhythm check and everything `f64` (`intervals`, `Spread`, `consistency`, the
WPM arithmetic and its rounding). `into_scored`'s speed cap is modelled with `underCap`, the
exact integer form of its float comparison.

The theorems, in the order the proofs build them: `Inv` is the representation invariant that
keeps every index and slice of the Rust in bounds (`init_inv`, `step_inv`); `absStep_lock` and
`step_lock` are the lock on a correct committed word; `Inv.credit` is the WPM credit of the
committed words; `step_keys` and `charKeys_ge_of_finished` count the recorded keystrokes, and
`tooFast_sound` turns the count into the `TooFast` answer for time tests; `run_ok` collects
what holds of every accepted replay, including the per-second marks.

Abstractions:

- `Nat` for `u32` and `usize`. Keystroke counters stay below `MAX_KEYSTROKES` (10,000) and
  timestamps are bounded by the time limit plus tolerance, so no `u32` arithmetic here can wrap.
  `limit_ms + LATE_KEY_TOLERANCE_MS` is at most 120,100.
- `List Char` for `String`. `word_start` and `rfind` are byte offsets in the Rust; every offset
  the Rust takes lands on a space or the end of the buffer, which are char boundaries, so the
  model's char positions correspond one to one (`Replay.Inv` names the positions).
- `targets` is a parameter of each function rather than a field, as the Rust borrows it
  immutably. It is a nonempty list of nonempty words without spaces.
- `key_tallies` is a function from the expected key to its tally instead of a 128-entry array.
-/

namespace Platen

/-- `BACKSPACE`. -/
def backspaceKey : Char := Char.ofNat 8

/-- `LATE_KEY_TOLERANCE_MS`. -/
def lateKeyToleranceMs : Nat := 100

/-- `Rejection`. -/
inductive Rejection
  | empty | tooManyKeystrokes | lengthMismatch | timesNotMonotonic | exceedsTimeLimit
  | incomplete | extraInput | tooFast | inhumanRhythm | submittedTooEarly | expired
  deriving DecidableEq, Repr

/-- `KeyTally`. -/
structure KeyTally where
  hits : Nat := 0
  misses : Nat := 0
  deriving DecidableEq, Repr

/-- Applies `f` to the last element, as `Vec::last_mut` does. No-op on an empty list. -/
def updateLast (f : α → α) : List α → List α
  | [] => []
  | [a] => [f a]
  | a :: b :: rest => a :: updateLast f (b :: rest)

/-- `str::rfind(' ')`: the index of the last space. -/
def rfindSpace : List Char → Option Nat
  | [] => none
  | c :: rest =>
    match rfindSpace rest with
    | some i => some (i + 1)
    | none => if c = ' ' then some 0 else none

/-- `str::split(' ')`. -/
def splitSpaces : List Char → List (List Char)
  | [] => [[]]
  | c :: rest =>
    if c = ' ' then [] :: splitSpaces rest
    else
      match splitSpaces rest with
      | piece :: pieces => (c :: piece) :: pieces
      | [] => [[c]]

/-- `str::split_terminator(' ')`: `split`, minus a final empty piece. -/
def splitTerminator (s : List Char) : List (List Char) :=
  let pieces := splitSpaces s
  if pieces.getLast? = some [] then pieces.dropLast else pieces

/-- `.map_or(0, |space| space + 1)` on `rfind`: where the word after the last space starts. -/
def previousStartOf (s : List Char) : Nat :=
  match rfindSpace s with
  | some space => space + 1
  | none => 0

/-- Words joined with one space after each, the shape of `typed` up to `word_start`. -/
def joinSp : List (List Char) → List Char
  | [] => []
  | w :: ws => w ++ ' ' :: joinSp ws

/-- `Replay`. -/
structure Replay where
  typed : List Char
  wordStart : Nat
  wordIndex : Nat
  wordChars : Nat
  committedCorrectChars : Nat
  correctKeystrokes : Nat
  incorrectKeystrokes : Nat
  keyTallies : Char → KeyTally
  seconds : List SecondMark
  elapsedMs : Nat
  finished : Bool
  endedOnSpace : Bool

namespace Replay

/-- The fresh replay `Replay::run` starts from. -/
def init : Replay :=
  { typed := [], wordStart := 0, wordIndex := 0, wordChars := 0, committedCorrectChars := 0,
    correctKeystrokes := 0, incorrectKeystrokes := 0, keyTallies := fun _ => {}, seconds := [],
    elapsedMs := 0, finished := false, endedOnSpace := false }

/-- Character keystrokes the replay has recorded. -/
def keys (r : Replay) : Nat := r.correctKeystrokes + r.incorrectKeystrokes

/-- `Replay::current_word`. -/
def currentWord (r : Replay) : List Char := r.typed.drop r.wordStart

/-- `Replay::target`. The default is never used: `Inv.inBounds`. -/
def target (targets : List (List Char)) (r : Replay) : List Char := (targets[r.wordIndex]?).getD []

/-- `Replay::is_last_word`. -/
def isLastWord (targets : List (List Char)) (r : Replay) : Bool :=
  r.wordIndex + 1 == targets.length

/-- The tally map after `f` is applied to the expected key's entry, if there is one. -/
def tally (r : Replay) (expected : Option Char) (f : KeyTally → KeyTally) : Char → KeyTally :=
  fun k => if expected = some k then f (r.keyTallies k) else r.keyTallies k

/-- The mark update at the end of `Replay::record`: `mark.chars += 1` and
`mark.errors += u32::from(!correct)`. -/
def recordMark (correct : Bool) (m : SecondMark) : SecondMark :=
  { m with chars := m.chars + 1, errors := m.errors + if correct then 0 else 1 }

/-- `Replay::record`. The mark update is unconditional in the Rust and comes after the
branch; `seconds` is non-empty there because the loop marked the key's second first. -/
def record (r : Replay) (correct : Bool) (expected : Option Char) : Replay :=
  let r :=
    if correct then
      { r with
        correctKeystrokes := r.correctKeystrokes + 1
        keyTallies := r.tally expected fun t => { t with hits := t.hits + 1 } }
    else
      { r with
        incorrectKeystrokes := r.incorrectKeystrokes + 1
        keyTallies := r.tally expected fun t => { t with misses := t.misses + 1 } }
  { r with seconds := updateLast (recordMark correct) r.seconds }

/-- `Replay::character`. -/
def character (targets : List (List Char)) (r : Replay) (c : Char) : Replay × Bool :=
  let expected := (r.target targets)[r.wordChars]?
  let r := r.record (expected == some c) expected
  let r := { r with typed := r.typed ++ [c], wordChars := r.wordChars + 1 }
  (r, r.isLastWord targets && r.currentWord == r.target targets)

/-- `Replay::space`. The Rust's `if correct { committed_correct_chars += n }` is written as
`+= if correct then n else 0`. -/
def space (targets : List (List Char)) (r : Replay) : Replay × Bool :=
  if r.wordChars = 0 then (r, false)
  else
    let expected := ((r.target targets)[r.wordChars]?).getD ' '
    let correct := r.currentWord == r.target targets
    let r := r.record correct (some expected)
    if r.isLastWord targets then (r, true)
    else
      let typed := r.typed ++ [' ']
      ({ r with
          committedCorrectChars :=
            r.committedCorrectChars + if correct then r.wordChars + 1 else 0
          typed, wordStart := typed.length, wordIndex := r.wordIndex + 1, wordChars := 0 },
        false)

/-- `Replay::backspace`. -/
def backspace (targets : List (List Char)) (r : Replay) : Replay :=
  if r.wordChars > 0 then { r with typed := r.typed.dropLast, wordChars := r.wordChars - 1 }
  else if r.wordIndex = 0 then r
  else
    let previousEnd := r.wordStart - 1
    let previousStart := previousStartOf (r.typed.take previousEnd)
    let previous := (r.typed.take previousEnd).drop previousStart
    if previous == (targets[r.wordIndex - 1]?).getD [] then r
    else
      { r with
        wordChars := previous.length
        typed := r.typed.dropLast
        wordIndex := r.wordIndex - 1
        wordStart := previousStart }

/-- The `match key` dispatch in `Replay::run`'s loop: one key through the word bookkeeping. -/
def step (targets : List (List Char)) (r : Replay) (key : Char) : Replay × Bool :=
  if key = backspaceKey then (r.backspace targets, false)
  else if key = ' ' then r.space targets
  else r.character targets key

/-- `Replay::committed_words`. -/
def committedWords (targets : List (List Char)) (r : Replay) : List (List Char × List Char) :=
  let last := if r.finished then [(r.currentWord, r.target targets)] else []
  (splitTerminator (r.typed.take r.wordStart)).zip targets ++ last

/-- `Replay::correct_chars`. -/
def correctChars (targets : List (List Char)) (r : Replay) : Nat :=
  let current := r.currentWord
  let credited :=
    if r.finished then current == r.target targets else current.isPrefixOf (r.target targets)
  r.committedCorrectChars + if credited then current.length else 0

/-- `Replay::final_chars`. -/
def finalChars (r : Replay) : Nat := r.typed.length + if r.endedOnSpace then 1 else 0

/-- The mark bookkeeping at the top of `Replay::run`'s loop body: opens a mark for this key's
second unless the last mark already is one. -/
def markSecond (targets : List (List Char)) (r : Replay) (timeMs : Nat) : Replay :=
  let second := secondOf timeMs
  if r.seconds.getLast?.all (fun mark => mark.second != second) then
    { r with
      seconds := r.seconds ++
        [{ second, chars := 0, errors := 0, correctCharsBefore := r.correctChars targets }] }
  else r

/-- One iteration of `Replay::run`'s loop over `(replay, previous_ms, finished)`. A late key
within tolerance is skipped and leaves the state as it was. -/
def loopStep (limit : Option Nat) (targets : List (List Char)) (r : Replay) (previousMs : Nat)
    (fin : Bool) (key : Char) (timeMs : Nat) : Except Rejection (Replay × Nat × Bool) :=
  if fin then .error .extraInput
  else if timeMs < previousMs then .error .timesNotMonotonic
  else if limit.any (fun limitMs => timeMs > limitMs) then
    if limit.any (fun limitMs => timeMs > limitMs + lateKeyToleranceMs) then .error .exceedsTimeLimit
    else .ok (r, previousMs, fin)
  else
    let r := r.markSecond targets timeMs
    let next := r.step targets key
    if next.2 then
      if limit.isSome then .error .tooFast
      else .ok ({ next.1 with elapsedMs := timeMs, endedOnSpace := key == ' ' }, timeMs, true)
    else .ok (next.1, timeMs, false)

/-- The loop of `Replay::run`. -/
def runLoop (limit : Option Nat) (targets : List (List Char)) :
    List (Char × Nat) → Replay × Nat × Bool → Except Rejection (Replay × Nat × Bool)
  | [], st => .ok st
  | (key, timeMs) :: rest, (r, previousMs, fin) =>
    match loopStep limit targets r previousMs fin key timeMs with
    | .error e => .error e
    | .ok st => runLoop limit targets rest st

/-- `Replay::run`. `keys` and `timesMs` pair up one to one; `evaluate` has already rejected a
length mismatch. -/
def run (mode : TestMode) (targets : List (List Char)) (keys : List Char) (timesMs : List Nat) :
    Except Rejection Replay :=
  let limit := mode.timeLimitMs
  match runLoop limit targets (keys.zip timesMs) (init, 0, false) with
  | .error e => .error e
  | .ok (r, _, fin) =>
    match limit with
    | some limitMs => .ok { r with elapsedMs := limitMs, finished := fin }
    | none => if fin then .ok { r with finished := fin } else .error .incomplete

/-- The integer checks of `Replay::into_scored`: `Empty`, a zero duration, and the speed cap.
The rhythm check and the score arithmetic are not modelled. -/
def intoScoredChecks (r : Replay) : Except Rejection Unit :=
  if r.keys = 0 then .error .empty
  else if r.elapsedMs = 0 then .error .tooFast
  else if ¬ underCap r.keys r.elapsedMs then .error .tooFast
  else .ok ()

/-! ## Conformance checks

Mirrors of `scoring.rs` tests whose expectations are integers, on hand-made words. -/

/-- Total `chars` over the marks. -/
def sumChars (marks : List SecondMark) : Nat := (marks.map (·.chars)).sum

/-- Total `errors` over the marks. -/
def sumErrors (marks : List SecondMark) : Nat := (marks.map (·.errors)).sum

/-- Whether `result` is `Ok` and satisfies `p`. -/
def okWith (result : Except Rejection Replay) (p : Replay → Bool) : Bool :=
  match result with
  | .ok r => p r
  | .error _ => false

/-- Human-paced timestamps, 100 ms apart. -/
def paced (keys : List Char) : List Nat := (List.range keys.length).map (· * 100)

/-- Runs `keys` on `words`, 100 ms apart. -/
def runPaced (mode : TestMode) (words : List String) (keys : String) : Except Rejection Replay :=
  run mode (words.map String.toList) keys.toList (paced keys.toList)

/-! `backspace_returns_to_previous_word`: commit "ab" as "abx", back into it, fix, finish. -/
#guard okWith (runPaced .words10 ["ab", "cd"] "abx \x08\x08 cd") fun r =>
  r.incorrectKeystrokes == 2 && r.correctKeystrokes == 5 && r.finished &&
    r.correctChars [['a', 'b'], ['c', 'd']] == 5 && r.typed == "ab cd".toList

/-! `extra_letters_are_errors_without_an_expected_key`: the extra `q` is an error with no
expected key; only the space that commits the word is tallied as a miss. -/
#guard okWith (runPaced .words10 ["ab", "cd"] "abq cd") fun r =>
  r.incorrectKeystrokes == 2 && (r.keyTallies ' ').misses == 1 && (r.keyTallies 'q').misses == 0 &&
    (r.keyTallies 'q').hits == 0 && (r.keyTallies 'a').hits == 1

/-! `correct_word_is_locked_against_backspace`: re-committing a correct word earns nothing. -/
#guard
  let honest := runPaced .time60 ["ab", "cd"] "ab "
  let padded := runPaced .time60 ["ab", "cd"] "ab \x08 \x08 \x08"
  okWith honest fun h => okWith padded fun p =>
    p.correctKeystrokes == h.correctKeystrokes && p.incorrectKeystrokes == 0 &&
      p.committedCorrectChars == 3

/-! `rejects_input_after_word_test_ends`, `rejects_unfinished_word_test`,
`rejects_decreasing_times`, `rejects_keystroke_after_time_limit`,
`ignores_keystroke_just_after_time_limit`. -/
#guard runPaced .words10 ["ab", "cd"] "ab cd " matches .error .extraInput
#guard runPaced .words10 ["ab", "cd"] "ab c" matches .error .incomplete
#guard run .words10 [['a', 'b']] ['a', 'b'] [5, 3] matches .error .timesNotMonotonic
#guard run .time15 [['a'], ['b']] ['a', ' ', 'b'] [0, 100, 15_101] matches .error .exceedsTimeLimit
#guard okWith (run .time15 [['a'], ['b']] ['a', ' ', 'b'] [0, 100, 15_100]) fun r =>
  r.keys == 2 && r.elapsedMs == 15_000

/-! A time test that reaches the end of its text is `TooFast`. -/
#guard runPaced .time15 ["ab", "cd"] "ab cd" matches .error .tooFast

/-! `wrong_last_word_ends_test_without_credit`: the committed words pair with their targets. -/
#guard okWith (runPaced .words10 ["ab", "cd"] "ab c ") fun r =>
  r.committedWords [['a', 'b'], ['c', 'd']] == [(['a', 'b'], ['a', 'b']), (['c'], ['c', 'd'])] &&
    r.endedOnSpace && r.finalChars == 5 && r.correctChars [['a', 'b'], ['c', 'd']] == 3

/-! ## The representation invariant

`typed` is the committed words, each followed by one space, then the word in progress. -/

variable {targets committed done ws : List (List Char)} {current last w m : List Char}
  {r r' : Replay} {ok : Bool} {e : Option Char} {key c : Char}
  {t previousMs previousMs' s i : Nat} {fin fin' : Bool} {limit : Option Nat}
  {l : List SecondMark}

@[simp] theorem joinSp_nil : joinSp [] = [] := rfl

@[simp] theorem joinSp_cons : joinSp (w :: ws) = w ++ ' ' :: joinSp ws := rfl

theorem joinSp_append (a b : List (List Char)) : joinSp (a ++ b) = joinSp a ++ joinSp b := by
  induction a with
  | nil => rfl
  | cons w ws ih => simp [ih]

theorem joinSp_concat (ws : List (List Char)) (w : List Char) :
    joinSp (ws ++ [w]) = joinSp ws ++ w ++ [' '] := by
  simp [joinSp_append]

/-- Nonempty words with their spaces take at least two characters each. -/
theorem two_mul_length_le_joinSp (h : ∀ w ∈ ws, w ≠ []) :
    2 * ws.length ≤ (joinSp ws).length := by
  induction ws with
  | nil => simp
  | cons w ws ih =>
    simp only [List.mem_cons, forall_eq_or_imp] at h
    have := ih h.2
    have : w.length ≠ 0 := by simpa using h.1
    simp; omega

theorem rfindSpace_of_not_mem (h : ' ' ∉ m) : rfindSpace m = none := by
  induction m with
  | nil => rfl
  | cons c rest ih =>
    simp only [List.mem_cons, not_or] at h
    simp [rfindSpace, ih h.2, Ne.symm h.1]

theorem rfindSpace_append_space (l : List Char) (h : ' ' ∉ m) :
    rfindSpace (l ++ ' ' :: m) = some l.length := by
  induction l with
  | nil => simp [rfindSpace, rfindSpace_of_not_mem h]
  | cons c rest ih => simp [rfindSpace, ih]

/-- `rfind` on the committed words minus their last space lands exactly on the last word:
`backspace` reopens `last` and nothing else. -/
theorem previousStartOf_joinSp (hinit : ∀ w ∈ done, ' ' ∉ w) (hlast : ' ' ∉ last) :
    previousStartOf (joinSp done ++ last) = (joinSp done).length := by
  rcases List.eq_nil_or_concat done with rfl | ⟨done', w, rfl⟩
  · simp [previousStartOf, rfindSpace_of_not_mem hlast]
  · simp only [List.concat_eq_append, List.mem_append, List.mem_singleton] at hinit ⊢
    unfold previousStartOf
    rw [joinSp_concat, List.append_assoc (joinSp done' ++ w), List.singleton_append,
      rfindSpace_append_space _ hlast]
    simp; omega

theorem splitSpaces_word (tl : List Char) (h : ' ' ∉ w) :
    splitSpaces (w ++ ' ' :: tl) = w :: splitSpaces tl := by
  induction w with
  | nil => simp [splitSpaces]
  | cons c rest ih =>
    simp only [List.mem_cons, not_or] at h
    simp [splitSpaces, ih h.2, Ne.symm h.1]

theorem splitSpaces_joinSp (h : ∀ w ∈ ws, ' ' ∉ w) : splitSpaces (joinSp ws) = ws ++ [[]] := by
  induction ws with
  | nil => rfl
  | cons w ws ih =>
    simp only [List.mem_cons, forall_eq_or_imp] at h
    simp [splitSpaces_word _ h.1, ih h.2]

/-- `split_terminator` undoes the joining, so `committed_words` sees each committed word. -/
theorem splitTerminator_joinSp (h : ∀ w ∈ ws, ' ' ∉ w) : splitTerminator (joinSp ws) = ws := by
  simp [splitTerminator, splitSpaces_joinSp h]

/-- WPM credit of committed words: each one equal to its target earns its length plus one, for
its space. `index` is the target index of the first word. -/
def creditFrom (targets : List (List Char)) (index : Nat) : List (List Char) → Nat
  | [] => 0
  | w :: ws =>
    (if targets[index]? = some w then w.length + 1 else 0) + creditFrom targets (index + 1) ws

theorem creditFrom_concat (targets : List (List Char)) (i : Nat) (ws : List (List Char))
    (w : List Char) :
    creditFrom targets i (ws ++ [w]) =
      creditFrom targets i ws + if targets[i + ws.length]? = some w then w.length + 1 else 0 := by
  induction ws generalizing i with
  | nil => simp [creditFrom]
  | cons v vs ih => simp [creditFrom, ih, Nat.add_assoc, Nat.add_comm 1 vs.length]

/-- The representation invariant of `Replay`, relative to the abstract state: the words
`committed` so far and the `current` word in progress. -/
structure Inv (targets committed : List (List Char)) (current : List Char) (r : Replay) :
    Prop where
  typed : r.typed = joinSp committed ++ current
  wordStart : r.wordStart = (joinSp committed).length
  wordChars : r.wordChars = current.length
  wordIndex : r.wordIndex = committed.length
  inBounds : committed.length < targets.length
  words : ∀ w ∈ committed, w ≠ [] ∧ ' ' ∉ w
  noSpace : ' ' ∉ current
  credit : r.committedCorrectChars = creditFrom targets 0 committed
  keys : r.typed.length ≤ r.keys

theorem init_inv (h : targets ≠ []) : Inv targets [] [] init where
  typed := rfl
  wordStart := rfl
  wordChars := rfl
  wordIndex := rfl
  inBounds := by cases targets with
    | nil => exact absurd rfl h
    | cons => simp
  words := by simp
  noSpace := by simp
  credit := rfl
  keys := by simp [init, keys]

/-- The invariant depends only on the word bookkeeping fields and the key count. -/
theorem Inv.congr (h : Inv targets committed current r) (e₁ : r'.typed = r.typed)
    (e₂ : r'.wordStart = r.wordStart) (e₃ : r'.wordChars = r.wordChars)
    (e₄ : r'.wordIndex = r.wordIndex) (e₅ : r'.committedCorrectChars = r.committedCorrectChars)
    (e₆ : r'.keys = r.keys) : Inv targets committed current r' where
  typed := e₁ ▸ h.typed
  wordStart := e₂ ▸ h.wordStart
  wordChars := e₃ ▸ h.wordChars
  wordIndex := e₄ ▸ h.wordIndex
  inBounds := h.inBounds
  words := h.words
  noSpace := h.noSpace
  credit := e₅ ▸ h.credit
  keys := by rw [e₁, e₆]; exact h.keys

/-- The in-bounds facts behind the Rust's indexing and slicing. -/
theorem Inv.wordStart_le (h : Inv targets committed current r) : r.wordStart ≤ r.typed.length := by
  simp [h.typed, h.wordStart]

theorem Inv.currentWord (h : Inv targets committed current r) : r.currentWord = current := by
  simp [Replay.currentWord, h.typed, h.wordStart]

theorem Inv.target (h : Inv targets committed current r) :
    r.target targets = (targets[committed.length]?).getD [] := by
  simp [Replay.target, h.wordIndex]

theorem Inv.isLastWord (h : Inv targets committed current r) :
    r.isLastWord targets = (committed.length + 1 == targets.length) := by
  simp [Replay.isLastWord, h.wordIndex]

/-- `word_start - 1` cannot underflow: with a word committed, `typed` holds its space. -/
theorem Inv.one_le_wordStart (h : Inv targets committed current r) (hc : committed ≠ []) :
    1 ≤ r.wordStart := by
  rcases List.eq_nil_or_concat committed with rfl | ⟨done, last, rfl⟩
  · exact absurd rfl hc
  · simp [h.wordStart, joinSp_concat]; omega

/-! ### `record` -/

theorem record_typed : (r.record ok e).typed = r.typed := by cases ok <;> rfl
theorem record_wordStart : (r.record ok e).wordStart = r.wordStart := by cases ok <;> rfl
theorem record_wordChars : (r.record ok e).wordChars = r.wordChars := by cases ok <;> rfl
theorem record_wordIndex : (r.record ok e).wordIndex = r.wordIndex := by cases ok <;> rfl
theorem record_committedCorrectChars :
    (r.record ok e).committedCorrectChars = r.committedCorrectChars := by cases ok <;> rfl
theorem record_elapsedMs : (r.record ok e).elapsedMs = r.elapsedMs := by cases ok <;> rfl
theorem record_finished : (r.record ok e).finished = r.finished := by cases ok <;> rfl
theorem record_endedOnSpace : (r.record ok e).endedOnSpace = r.endedOnSpace := by cases ok <;> rfl
theorem record_keys : (r.record ok e).keys = r.keys + 1 := by
  cases ok <;> simp [record, keys] <;> omega
theorem record_correct :
    (r.record ok e).correctKeystrokes = r.correctKeystrokes + if ok then 1 else 0 := by
  cases ok <;> simp [record]
theorem record_incorrect :
    (r.record ok e).incorrectKeystrokes = r.incorrectKeystrokes + if ok then 0 else 1 := by
  cases ok <;> simp [record]
theorem record_seconds : (r.record ok e).seconds = updateLast (recordMark ok) r.seconds := by
  cases ok <;> rfl
theorem record_isLastWord : (r.record ok e).isLastWord targets = r.isLastWord targets := by
  simp [isLastWord, record_wordIndex]
theorem record_currentWord : (r.record ok e).currentWord = r.currentWord := by
  simp [currentWord, record_typed, record_wordStart]
theorem record_target : (r.record ok e).target targets = r.target targets := by
  simp [target, record_wordIndex]

attribute [simp] record_typed record_wordStart record_wordChars record_wordIndex
  record_committedCorrectChars record_elapsedMs record_finished record_endedOnSpace record_keys
  record_correct record_incorrect record_seconds record_isLastWord record_currentWord record_target

/-! ### `character` -/

theorem character_fst :
    (r.character targets c).1 =
      { r.record ((r.target targets)[r.wordChars]? == some c) (r.target targets)[r.wordChars]? with
        typed := r.typed ++ [c], wordChars := r.wordChars + 1 } := by
  simp [character]

theorem character_snd (h : Inv targets committed current r) :
    (r.character targets c).2 =
      (committed.length + 1 == targets.length &&
        current ++ [c] == (targets[committed.length]?).getD []) := by
  simp [character, isLastWord, currentWord, target, h.typed, h.wordStart, h.wordIndex,
    List.drop_left']

theorem character_inv (h : Inv targets committed current r) (hc : c ≠ ' ') :
    Inv targets committed (current ++ [c]) (r.character targets c).1 where
  typed := by simp [character_fst, h.typed]
  wordStart := by simp [character_fst, h.wordStart]
  wordChars := by simp [character_fst, h.wordChars]
  wordIndex := by simp [character_fst, h.wordIndex]
  inBounds := h.inBounds
  words := h.words
  noSpace := by simp [h.noSpace, Ne.symm hc]
  credit := by simp [character_fst, h.credit]
  keys := by
    have := h.keys
    rw [character_fst]
    simp only [keys, List.length_append, List.length_singleton, record_correct,
      record_incorrect] at this ⊢
    cases ((r.target targets)[r.wordChars]? == some c) <;> simp <;> omega

/-! ### `space` -/

theorem space_ignored (h : r.wordChars = 0) : r.space targets = (r, false) := by
  simp [space, h]

theorem space_last_fst (h0 : r.wordChars ≠ 0) (hl : r.isLastWord targets = true) :
    (r.space targets).1 =
      r.record (r.currentWord == r.target targets)
        (some (((r.target targets)[r.wordChars]?).getD ' ')) := by
  simp [space, h0, hl]

theorem space_last_snd (h0 : r.wordChars ≠ 0) (hl : r.isLastWord targets = true) :
    (r.space targets).2 = true := by
  simp [space, h0, hl]

theorem space_commit_fst (h0 : r.wordChars ≠ 0) (hl : r.isLastWord targets = false) :
    (r.space targets).1 =
      { r.record (r.currentWord == r.target targets)
          (some (((r.target targets)[r.wordChars]?).getD ' ')) with
        committedCorrectChars :=
          r.committedCorrectChars +
            if (r.currentWord == r.target targets) then r.wordChars + 1 else 0
        typed := r.typed ++ [' ']
        wordStart := (r.typed ++ [' ']).length
        wordIndex := r.wordIndex + 1
        wordChars := 0 } := by
  simp [space, h0, hl]

theorem space_commit_snd (h0 : r.wordChars ≠ 0) (hl : r.isLastWord targets = false) :
    (r.space targets).2 = false := by
  simp [space, h0, hl]

/-- A space on the last word ends the test and leaves the words as they are. -/
theorem space_last_inv (h : Inv targets committed current r) (hne : current ≠ [])
    (hl : committed.length + 1 = targets.length) :
    Inv targets committed current (r.space targets).1 ∧ (r.space targets).2 = true := by
  have h0 : r.wordChars ≠ 0 := by simp [h.wordChars, hne]
  have hl' : r.isLastWord targets = true := by simp [h.isLastWord, hl]
  refine ⟨?_, space_last_snd h0 hl'⟩
  rw [space_last_fst h0 hl']
  exact {
    typed := by simp [h.typed]
    wordStart := by simp [h.wordStart]
    wordChars := by simp [h.wordChars]
    wordIndex := by simp [h.wordIndex]
    inBounds := h.inBounds
    words := h.words
    noSpace := h.noSpace
    credit := by simp [h.credit]
    keys := by have := h.keys; simp [record_keys]; omega }

/-- A space elsewhere commits the current word, crediting it only when it is right. -/
theorem space_commit_inv (h : Inv targets committed current r) (hne : current ≠ [])
    (hl : committed.length + 1 ≠ targets.length) :
    Inv targets (committed ++ [current]) [] (r.space targets).1 ∧
      (r.space targets).2 = false := by
  have h0 : r.wordChars ≠ 0 := by simp [h.wordChars, hne]
  have hl' : r.isLastWord targets = false := by simp [h.isLastWord, hl]
  refine ⟨?_, space_commit_snd h0 hl'⟩
  rw [space_commit_fst h0 hl']
  have hcur := h.currentWord
  have htgt := h.target
  exact {
    typed := by simp [h.typed, joinSp_concat]
    wordStart := by simp [h.typed, joinSp_concat]
    wordChars := by simp
    wordIndex := by simp [h.wordIndex]
    inBounds := by have := h.inBounds; simp; omega
    words := by
      simp only [List.mem_append, List.mem_singleton]
      rintro w (hw | rfl)
      · exact h.words w hw
      · exact ⟨hne, h.noSpace⟩
    noSpace := by simp
    credit := by
      obtain ⟨tgt, htgt'⟩ : ∃ tgt, targets[committed.length]? = some tgt :=
        ⟨_, List.getElem?_eq_getElem h.inBounds⟩
      simp only [creditFrom_concat, Nat.zero_add, htgt']
      rw [h.credit, hcur, htgt, h.wordChars, htgt']
      simp only [Option.getD_some, Option.some.injEq, beq_iff_eq]
      by_cases hw : current = tgt
      · simp [hw]
      · simp [hw, Ne.symm hw]
    keys := by
      have := h.keys
      simp only [keys, List.length_append, List.length_singleton, record_correct,
        record_incorrect] at this ⊢
      generalize (r.currentWord == r.target targets) = ok at *
      cases ok <;> simp <;> omega }

/-! ### `backspace` -/

theorem backspace_char (h : 0 < r.wordChars) :
    r.backspace targets = { r with typed := r.typed.dropLast, wordChars := r.wordChars - 1 } := by
  simp [backspace, h]

theorem backspace_first (h0 : r.wordChars = 0) (h1 : r.wordIndex = 0) :
    r.backspace targets = r := by
  simp [backspace, h0, h1]

/-- Deleting a letter of the current word. -/
theorem backspace_char_inv (h : Inv targets committed current r) (hne : current ≠ []) :
    Inv targets committed current.dropLast (r.backspace targets) := by
  have h0 : 0 < r.wordChars := by
    rw [h.wordChars]
    cases current with
    | nil => exact absurd rfl hne
    | cons => simp
  rw [backspace_char h0]
  rcases List.eq_nil_or_concat current with rfl | ⟨cur, c, rfl⟩
  · exact absurd rfl hne
  · simp only [List.concat_eq_append] at h ⊢
    have hsp : ' ' ∉ cur := fun hm => h.noSpace (by simp [hm])
    exact {
      typed := by simp [h.typed, ← List.append_assoc]
      wordStart := h.wordStart
      wordChars := by simp [h.wordChars]
      wordIndex := h.wordIndex
      inBounds := h.inBounds
      words := h.words
      noSpace := by simpa using hsp
      credit := h.credit
      keys := by have := h.keys; simp [keys, h.typed] at this ⊢; omega }

/-- The previous word as `backspace` slices it, and where it starts. -/
theorem backspace_previous (h : Inv targets (done ++ [last]) [] r) :
    r.typed.take (r.wordStart - 1) = joinSp done ++ last ∧
      previousStartOf (r.typed.take (r.wordStart - 1)) = (joinSp done).length := by
  have hw := h.words
  simp only [List.mem_append, List.mem_singleton] at hw
  have hinit : ∀ w ∈ done, ' ' ∉ w := fun w hw' => (hw w (Or.inl hw')).2
  have hlast : ' ' ∉ last := (hw last (Or.inr rfl)).2
  have ht : r.typed.take (r.wordStart - 1) = joinSp done ++ last := by
    rw [h.typed, h.wordStart, joinSp_concat, List.append_nil]
    simp only [List.length_append, List.length_singleton, Nat.add_sub_cancel]
    exact List.take_left' (by simp)
  exact ⟨ht, by rw [ht]; exact previousStartOf_joinSp hinit hlast⟩

/-- A correct previous word is locked: backspace at the start of the next word does nothing. -/
theorem backspace_locked (h : Inv targets (done ++ [last]) [] r)
    (hl : last = (targets[done.length]?).getD []) : r.backspace targets = r := by
  have h0 : ¬ 0 < r.wordChars := by simp [h.wordChars]
  have h1 : r.wordIndex ≠ 0 := by simp [h.wordIndex]
  obtain ⟨ht, hs⟩ := backspace_previous h
  have hs' := ht ▸ hs
  subst hl
  simp [backspace, h0, ht, hs', h.wordIndex]

/-- A wrong previous word is reopened: it becomes the current word again. -/
theorem backspace_reopen_inv (h : Inv targets (done ++ [last]) [] r)
    (hl : last ≠ (targets[done.length]?).getD []) :
    Inv targets done last (r.backspace targets) := by
  have h0 : ¬ 0 < r.wordChars := by simp [h.wordChars]
  have h1 : r.wordIndex ≠ 0 := by simp [h.wordIndex]
  obtain ⟨ht, hs⟩ := backspace_previous h
  have hs' := ht ▸ hs
  have hw := h.words
  simp only [List.mem_append, List.mem_singleton] at hw
  simp only [backspace, h0, ht, hs', List.drop_left, h.wordIndex, hl, ite_false, beq_iff_eq,
    List.length_append, List.length_singleton, Nat.add_sub_cancel, Nat.add_one_ne_zero]
  exact {
    typed := by simp only [h.typed, joinSp_concat, List.append_nil, List.dropLast_concat]
    wordStart := rfl
    wordChars := by simp
    wordIndex := rfl
    inBounds := by have := h.inBounds; simp at this; omega
    words := fun w hw' => hw w (Or.inl hw')
    noSpace := (hw last (Or.inr rfl)).2
    credit := by
      have := h.credit
      obtain ⟨tgt, htgt'⟩ : ∃ tgt, targets[done.length]? = some tgt :=
        ⟨_, List.getElem?_eq_getElem (by have := h.inBounds; simp at this; omega)⟩
      rw [creditFrom_concat, Nat.zero_add, htgt'] at this
      rw [htgt'] at hl
      simp only [Option.getD_some] at hl
      simpa [Ne.symm hl] using this
    keys := by
      have := h.keys
      simp only [keys, h.typed, joinSp_concat, List.append_nil, List.length_append,
        List.length_singleton, List.dropLast_concat] at this ⊢
      omega }

/-! ### One key on the abstract state -/

/-- What `step` does to the words, stated on the abstract state. -/
def absStep (targets committed : List (List Char)) (current : List Char) (key : Char) :
    (List (List Char) × List Char) × Bool :=
  if key = backspaceKey then
    if current ≠ [] then ((committed, current.dropLast), false)
    else
      match committed.getLast? with
      | none => ((committed, current), false)
      | some last =>
        if last = (targets[committed.length - 1]?).getD [] then ((committed, current), false)
        else ((committed.dropLast, last), false)
  else if key = ' ' then
    if current = [] then ((committed, current), false)
    else if committed.length + 1 = targets.length then ((committed, current), true)
    else ((committed ++ [current], []), false)
  else
    ((committed, current ++ [key]),
      committed.length + 1 == targets.length &&
        current ++ [key] == (targets[committed.length]?).getD [])

theorem space_ne_backspaceKey : ' ' ≠ backspaceKey := by decide

/-- Every key keeps the invariant, and the abstract state moves as `absStep` says. -/
theorem step_inv (h : Inv targets committed current r) (key : Char) :
    Inv targets (absStep targets committed current key).1.1
      (absStep targets committed current key).1.2 (r.step targets key).1 ∧
      (r.step targets key).2 = (absStep targets committed current key).2 := by
  unfold step absStep
  by_cases hb : key = backspaceKey
  · rw [ite_eq_left hb, ite_eq_left hb]
    by_cases hne : current = []
    · rw [ite_eq_right (fun h' => h' hne)]
      subst hne
      rcases List.eq_nil_or_concat committed with rfl | ⟨done, last, rfl⟩
      · simp only [List.getLast?_nil]
        rw [backspace_first (by simp [h.wordChars]) (by simp [h.wordIndex])]
        exact ⟨h, by simp⟩
      · simp only [List.concat_eq_append] at h ⊢
        simp only [List.getLast?_concat, List.length_append, List.length_singleton,
          Nat.add_sub_cancel]
        by_cases hl : last = (targets[done.length]?).getD []
        · rw [ite_eq_left hl, backspace_locked h hl]; exact ⟨h, rfl⟩
        · rw [ite_eq_right hl, List.dropLast_concat]; exact ⟨backspace_reopen_inv h hl, rfl⟩
    · rw [ite_eq_left hne]
      exact ⟨backspace_char_inv h hne, rfl⟩
  · rw [ite_eq_right hb, ite_eq_right hb]
    by_cases hs : key = ' '
    · rw [ite_eq_left hs, ite_eq_left hs]
      subst hs
      by_cases hne : current = []
      · rw [ite_eq_left hne]
        subst hne
        rw [space_ignored (by simp [h.wordChars])]
        exact ⟨h, rfl⟩
      · rw [ite_eq_right hne]
        by_cases hl : committed.length + 1 = targets.length
        · rw [ite_eq_left hl]; exact space_last_inv h hne hl
        · rw [ite_eq_right hl]; exact space_commit_inv h hne hl
    · rw [ite_eq_right hs, ite_eq_right hs]
      exact ⟨character_inv h hs, character_snd h⟩

/-! ## The lock: a correct committed word is never reopened or changed -/

/-- `absStep` only ever drops a wrong last word or appends; every word equal to its target
stays where it is. -/
theorem absStep_lock (hi : i < committed.length) (hw : committed[i]? = targets[i]?) :
    (absStep targets committed current key).1.1[i]? = committed[i]? := by
  unfold absStep
  by_cases hb : key = backspaceKey
  · rw [ite_eq_left hb]
    by_cases hne : current ≠ []
    · rw [ite_eq_left hne]
    · rw [ite_eq_right hne]
      rcases List.eq_nil_or_concat committed with rfl | ⟨done, last, rfl⟩
      · simp at hi
      · simp only [List.concat_eq_append] at hi hw ⊢
        simp only [List.getLast?_concat, List.length_append, List.length_singleton,
          Nat.add_sub_cancel]
        by_cases hl : last = (targets[done.length]?).getD []
        · rw [ite_eq_left hl]
        · rw [ite_eq_right hl, List.dropLast_concat]
          simp only [List.length_append, List.length_singleton] at hi
          rcases Nat.lt_or_ge i done.length with hlt | hge
          · rw [List.getElem?_append_left hlt]
          · have : i = done.length := by omega
            subst this
            rw [List.getElem?_concat_length] at hw
            have : (targets[done.length]?).getD [] = last := by rw [← hw]; rfl
            exact absurd this.symm hl
  · rw [ite_eq_right hb]
    by_cases hs : key = ' '
    · rw [ite_eq_left hs]
      by_cases hne : current = []
      · rw [ite_eq_left hne]
      · rw [ite_eq_right hne]
        by_cases hl : committed.length + 1 = targets.length
        · rw [ite_eq_left hl]
        · rw [ite_eq_right hl]; simp [List.getElem?_append_left hi]
    · rw [ite_eq_right hs]

/-- On the concrete replay: once a committed word equals its target, no later key
reopens or changes it. -/
theorem step_lock (h : Inv targets committed current r) (hi : i < committed.length)
    (hw : committed[i]? = targets[i]?) (key : Char) :
    ∃ committed' current', Inv targets committed' current' (r.step targets key).1 ∧
      committed'[i]? = committed[i]? :=
  ⟨_, _, (step_inv h key).1, absStep_lock hi hw⟩

/-- `committed_correct_chars` is the credit of the committed words that equal their
targets, each counted with its space. Under the invariant it is a projection. -/
theorem committedCorrectChars_eq (h : Inv targets committed current r) :
    r.committedCorrectChars = creditFrom targets 0 committed :=
  h.credit

/-! ## Keystroke counts -/

/-- What `backspace` leaves alone: everything but the word bookkeeping. -/
theorem backspace_frame :
    (r.backspace targets).committedCorrectChars = r.committedCorrectChars ∧
      (r.backspace targets).correctKeystrokes = r.correctKeystrokes ∧
      (r.backspace targets).incorrectKeystrokes = r.incorrectKeystrokes ∧
      (r.backspace targets).seconds = r.seconds ∧
      (r.backspace targets).elapsedMs = r.elapsedMs ∧
      (r.backspace targets).finished = r.finished ∧
      (r.backspace targets).endedOnSpace = r.endedOnSpace := by
  simp only [backspace]
  split
  · exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩
  · split
    · exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩
    · split <;> exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem backspace_keys : (r.backspace targets).keys = r.keys := by
  obtain ⟨-, h₂, h₃, -⟩ := backspace_frame (r := r) (targets := targets)
  simp [keys, h₂, h₃]

/-- `correct + incorrect` counts exactly the keys the replay records, which are
every key except a backspace and a space typed before any letter of a word. -/
theorem step_keys (h : Inv targets committed current r) (key : Char) :
    (r.step targets key).1.keys =
      r.keys + if key = backspaceKey ∨ (key = ' ' ∧ current = []) then 0 else 1 := by
  unfold step
  by_cases hb : key = backspaceKey
  · rw [ite_eq_left hb, ite_eq_left (Or.inl hb), Nat.add_zero]
    exact backspace_keys
  · rw [ite_eq_right hb]
    by_cases hs : key = ' '
    · rw [ite_eq_left hs]
      by_cases hne : current = []
      · rw [space_ignored (by simp [h.wordChars, hne]), ite_eq_left (Or.inr ⟨hs, hne⟩)]
        rfl
      · have h0 : r.wordChars ≠ 0 := by simp [h.wordChars, hne]
        rw [ite_eq_right (by simp [hb, hne])]
        cases hl : r.isLastWord targets
        · rw [space_commit_fst h0 hl]; exact record_keys
        · rw [space_last_fst h0 hl]; exact record_keys
    · rw [ite_eq_right hs, ite_eq_right (by simp [hb, hs]), character_fst]
      exact record_keys

/-- The keys needed to reach the end of the text: one letter per committed word plus its
space, and at least one key of the last word. `Platen.time_text_outlasts_cap` turns this into
the `TooFast` answer for time tests. -/
theorem charKeys_ge_of_finished (h : Inv targets committed current r)
    (hfin : (r.step targets key).2 = true) :
    minKeystrokesToFinish targets.length ≤ (r.step targets key).1.keys := by
  have hk := step_keys h key
  have hfin' : (absStep targets committed current key).2 = true := by
    rw [← (step_inv h key).2]; exact hfin
  have hwords : ∀ w ∈ committed, w ≠ [] := fun w hw => (h.words w hw).1
  have hjoin := two_mul_length_le_joinSp hwords
  have hkeys := h.keys
  rw [h.typed] at hkeys
  simp only [List.length_append] at hkeys
  unfold absStep at hfin'
  rw [hk]
  unfold minKeystrokesToFinish minKeystrokesPerWord
  by_cases hb : key = backspaceKey
  · rw [ite_eq_left hb] at hfin'
    by_cases hne : current ≠ []
    · rw [ite_eq_left hne] at hfin'; simp at hfin'
    · rw [ite_eq_right hne] at hfin'
      rcases List.eq_nil_or_concat committed with rfl | ⟨done, last, rfl⟩
      · simp at hfin'
      · simp only [List.concat_eq_append, List.getLast?_concat] at hfin'
        split at hfin' <;> simp at hfin'
  · rw [ite_eq_right hb] at hfin'
    by_cases hs : key = ' '
    · rw [ite_eq_left hs] at hfin'
      by_cases hne : current = []
      · rw [ite_eq_left hne] at hfin'; simp at hfin'
      · rw [ite_eq_right hne] at hfin'
        by_cases hl : committed.length + 1 = targets.length
        · have : current.length ≠ 0 := by simpa [List.length_eq_zero_iff] using hne
          simp [hb, hne]; omega
        · rw [ite_eq_right hl] at hfin'; simp at hfin'
    · rw [ite_eq_right hs] at hfin'
      simp only [Bool.and_eq_true, beq_iff_eq] at hfin'
      simp [hb, hs]
      omega

/-- With `time_text_outlasts_cap`: in a time test, reaching the end of the text means
the recorded keys are above the speed cap for the test's limit, so `Replay::run` answering
`TooFast` is what `into_scored` would have answered. -/
theorem tooFast_sound (m : TestMode) {limitMs : Nat} (hm : m.timeLimitMs = some limitMs)
    (hn : targets.length = m.wordCount) (h : Inv targets committed current r)
    (hfin : (r.step targets key).2 = true) : ¬ underCap (r.step targets key).1.keys limitMs := by
  intro hcap
  have := time_text_outlasts_cap m hm hcap
  have := charKeys_ge_of_finished h hfin
  rw [hn] at this
  omega

/-! ## What a key does to the rest of the replay -/

/-- Everything a key changes outside the word bookkeeping: either nothing, or one recorded
keystroke, right or wrong, counted on the last mark. -/
structure StepFrame (r r' : Replay) : Prop where
  elapsedMs : r'.elapsedMs = r.elapsedMs
  finished : r'.finished = r.finished
  endedOnSpace : r'.endedOnSpace = r.endedOnSpace
  marks : (r'.keys = r.keys ∧ r'.incorrectKeystrokes = r.incorrectKeystrokes ∧
      r'.seconds = r.seconds) ∨
    ∃ ok, r'.keys = r.keys + 1 ∧ r'.incorrectKeystrokes = r.incorrectKeystrokes + (if ok then 0 else 1) ∧
      r'.seconds = updateLast (recordMark ok) r.seconds

theorem record_frame : StepFrame r (r.record ok e) where
  elapsedMs := record_elapsedMs
  finished := record_finished
  endedOnSpace := record_endedOnSpace
  marks := Or.inr ⟨ok, record_keys, record_incorrect, record_seconds⟩

theorem step_frame : StepFrame r (r.step targets key).1 := by
  unfold step
  by_cases hb : key = backspaceKey
  · rw [ite_eq_left hb]
    obtain ⟨-, h₂, h₃, h₄, h₅, h₆, h₇⟩ := backspace_frame (r := r) (targets := targets)
    exact ⟨h₅, h₆, h₇, Or.inl ⟨by simp [keys, h₂, h₃], h₃, h₄⟩⟩
  · rw [ite_eq_right hb]
    by_cases hs : key = ' '
    · rw [ite_eq_left hs]
      by_cases h0 : r.wordChars = 0
      · rw [space_ignored h0]; exact ⟨rfl, rfl, rfl, Or.inl ⟨rfl, rfl, rfl⟩⟩
      · cases hl : r.isLastWord targets
        · rw [space_commit_fst h0 hl]
          exact ⟨record_elapsedMs, record_finished, record_endedOnSpace,
            Or.inr ⟨_, record_keys, record_incorrect, record_seconds⟩⟩
        · rw [space_last_fst h0 hl]; exact record_frame
    · rw [ite_eq_right hs, character_fst]
      exact ⟨record_elapsedMs, record_finished, record_endedOnSpace,
        Or.inr ⟨_, record_keys, record_incorrect, record_seconds⟩⟩

/-! ## Marks over the run

The marks ascend strictly by second, their `chars` add up to the recorded keys and
their `errors` to the wrong ones. -/

theorem updateLast_concat (f : α → α) (l : List α) (a : α) :
    updateLast f (l ++ [a]) = l ++ [f a] := by
  induction l with
  | nil => rfl
  | cons b l ih =>
    cases l with
    | nil => rfl
    | cons c l => simp [updateLast] at ih ⊢; exact ih

theorem map_updateLast (f : α → α) (g : α → β) (hf : ∀ a, g (f a) = g a) (l : List α) :
    (updateLast f l).map g = l.map g := by
  induction l with
  | nil => rfl
  | cons b l ih =>
    cases l with
    | nil => simp [updateLast, hf]
    | cons c l => simp [updateLast] at ih ⊢; exact ih

@[simp] theorem sumChars_append (a b : List SecondMark) :
    sumChars (a ++ b) = sumChars a + sumChars b := by
  simp [sumChars]

@[simp] theorem sumErrors_append (a b : List SecondMark) :
    sumErrors (a ++ b) = sumErrors a + sumErrors b := by
  simp [sumErrors]

theorem sumChars_record (hne : l ≠ []) :
    sumChars (updateLast (recordMark ok) l) = sumChars l + 1 := by
  rcases List.eq_nil_or_concat l with rfl | ⟨l', x, rfl⟩
  · exact absurd rfl hne
  · simp only [List.concat_eq_append]; simp [updateLast_concat, sumChars, recordMark]; omega

theorem sumErrors_record (hne : l ≠ []) :
    sumErrors (updateLast (recordMark ok) l) = sumErrors l + if ok then 0 else 1 := by
  rcases List.eq_nil_or_concat l with rfl | ⟨l', x, rfl⟩
  · exact absurd rfl hne
  · simp only [List.concat_eq_append]; simp [updateLast_concat, sumErrors, recordMark]; omega

theorem seconds_record : (updateLast (recordMark ok) l).map (·.second) = l.map (·.second) :=
  map_updateLast (recordMark ok) (·.second) (fun _ => rfl) l

/-- The loop invariant of `Replay::run`, over its state `(replay, previous_ms, finished)`. -/
structure LoopInv (targets : List (List Char)) (r : Replay) (previousMs : Nat) (fin : Bool) :
    Prop where
  abs : ∃ committed current, Inv targets committed current r
  ascending : (r.seconds.map (·.second)).Pairwise (· < ·)
  bounded : ∀ m ∈ r.seconds, m.second ≤ secondOf previousMs
  chars : sumChars r.seconds = r.keys
  errors : sumErrors r.seconds = r.incorrectKeystrokes
  ended : fin = false → r.endedOnSpace = false
  final : r.finalChars ≤ r.keys
  finishedKeys : fin = true → minKeystrokesToFinish targets.length ≤ r.keys

theorem init_loopInv (h : targets ≠ []) : LoopInv targets init 0 false where
  abs := ⟨[], [], init_inv h⟩
  ascending := by simp [init]
  bounded := by simp [init]
  chars := rfl
  errors := rfl
  ended := fun _ => rfl
  final := by simp [init, finalChars, keys]
  finishedKeys := by simp

theorem markSecond_typed : (r.markSecond targets t).typed = r.typed := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_wordStart : (r.markSecond targets t).wordStart = r.wordStart := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_wordChars : (r.markSecond targets t).wordChars = r.wordChars := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_wordIndex : (r.markSecond targets t).wordIndex = r.wordIndex := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_committedCorrectChars :
    (r.markSecond targets t).committedCorrectChars = r.committedCorrectChars := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_keys : (r.markSecond targets t).keys = r.keys := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_incorrect :
    (r.markSecond targets t).incorrectKeystrokes = r.incorrectKeystrokes := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_endedOnSpace : (r.markSecond targets t).endedOnSpace = r.endedOnSpace := by
  simp only [markSecond]; split <;> rfl
theorem markSecond_finalChars : (r.markSecond targets t).finalChars = r.finalChars := by
  simp [finalChars, markSecond_typed, markSecond_endedOnSpace]

theorem markSecond_inv (h : Inv targets committed current r) :
    Inv targets committed current (r.markSecond targets t) :=
  h.congr markSecond_typed markSecond_wordStart markSecond_wordChars markSecond_wordIndex
    markSecond_committedCorrectChars markSecond_keys

/-- The marks after `markSecond`: the old ones, with one more for this second unless the last
one already is. -/
theorem markSecond_seconds :
    (r.markSecond targets t).seconds = r.seconds ∨
      ((r.markSecond targets t).seconds =
        r.seconds ++ [⟨secondOf t, 0, 0, r.correctChars targets⟩] ∧
        r.seconds.getLast?.all (fun mark => mark.second != secondOf t) = true) := by
  simp only [markSecond]
  split
  · next hc => right; exact ⟨rfl, hc⟩
  · left; rfl

theorem markSecond_ne_nil : (r.markSecond targets t).seconds ≠ [] := by
  simp only [markSecond]
  split
  · exact List.append_ne_nil_of_right_ne_nil _ (by simp)
  · next hc =>
    rcases List.eq_nil_or_concat r.seconds with hr | ⟨l, x, hr⟩
    · simp [hr] at hc
    · simp [hr, List.concat_eq_append]

/-- In an ascending list whose last mark is not this second and whose marks are all at or
before it, every mark is strictly before it. -/
theorem lt_of_ascending (hp : (l.map (·.second)).Pairwise (· < ·))
    (hb : ∀ m ∈ l, m.second ≤ s) (hl : l.getLast?.all (fun mark => mark.second != s) = true) :
    ∀ m ∈ l, m.second < s := by
  rcases List.eq_nil_or_concat l with rfl | ⟨l', x, rfl⟩
  · exact fun _ hm => nomatch hm
  · simp only [List.concat_eq_append] at hp hb hl ⊢
    simp only [List.getLast?_concat, Option.all_some, bne_iff_ne, ne_eq] at hl
    have hx : x.second ≤ s := hb x (by simp)
    rw [List.map_append, List.pairwise_append] at hp
    intro m hm
    simp only [List.mem_append, List.mem_singleton] at hm
    rcases hm with hm | rfl
    · have := hp.2.2 m.second (List.mem_map_of_mem hm) x.second (by simp)
      omega
    · omega

theorem markSecond_marks (hp : (r.seconds.map (·.second)).Pairwise (· < ·))
    (hb : ∀ m ∈ r.seconds, m.second ≤ secondOf previousMs) (ht : previousMs ≤ t) :
    ((r.markSecond targets t).seconds.map (·.second)).Pairwise (· < ·) ∧
      (∀ m ∈ (r.markSecond targets t).seconds, m.second ≤ secondOf t) ∧
      sumChars (r.markSecond targets t).seconds = sumChars r.seconds ∧
      sumErrors (r.markSecond targets t).seconds = sumErrors r.seconds := by
  have hmono := secondOf_mono ht
  have hb' : ∀ m ∈ r.seconds, m.second ≤ secondOf t := fun m hm => by
    have := hb m hm; omega
  rcases markSecond_seconds (r := r) (targets := targets) (t := t) with he | ⟨he, hc⟩
  · rw [he]; exact ⟨hp, hb', rfl, rfl⟩
  · rw [he]
    have hlt := lt_of_ascending hp hb' hc
    refine ⟨?_, ?_, by simp [sumChars], by simp [sumErrors]⟩
    · rw [List.map_append, List.pairwise_append]
      refine ⟨hp, by simp, ?_⟩
      intro a ha b hb
      simp only [List.mem_map] at ha
      obtain ⟨m, hm, rfl⟩ := ha
      simp only [List.map_cons, List.map_nil, List.mem_singleton] at hb
      subst hb
      exact hlt m hm
    · intro m hm
      simp only [List.mem_append, List.mem_singleton] at hm
      rcases hm with hm | rfl
      · exact hb' m hm
      · exact Nat.le_refl _

/-- A space that ends the test records one key and leaves `typed` as it was: that space is
not stored, which is what `ended_on_space` is for. -/
theorem step_finished_space (hs : key = ' ')
    (hf : (r.step targets key).2 = true) :
    (r.step targets key).1.typed = r.typed ∧ (r.step targets key).1.keys = r.keys + 1 := by
  subst hs
  unfold step at hf ⊢
  rw [ite_eq_right space_ne_backspaceKey, ite_eq_left rfl] at hf ⊢
  by_cases h0 : r.wordChars = 0
  · rw [space_ignored h0] at hf; exact absurd hf (by simp)
  · cases hl : r.isLastWord targets
    · rw [space_commit_snd h0 hl] at hf; exact absurd hf (by simp)
    · rw [space_last_fst h0 hl]; exact ⟨record_typed, record_keys⟩

/-- One loop iteration keeps the invariant. -/
theorem loopStep_inv (h : LoopInv targets r previousMs fin)
    (hok : loopStep limit targets r previousMs fin key t = .ok (r', previousMs', fin')) :
    LoopInv targets r' previousMs' fin' := by
  simp only [loopStep] at hok
  split at hok
  · exact absurd hok (by simp)
  · next hfin =>
    split at hok
    · exact absurd hok (by simp)
    · next hlt =>
      split at hok
      · split at hok
        · exact absurd hok (by simp)
        · simp only [Except.ok.injEq, Prod.mk.injEq] at hok
          obtain ⟨rfl, rfl, rfl⟩ := hok
          exact h
      · have ht : previousMs ≤ t := Nat.le_of_not_lt hlt
        obtain ⟨committed, current, hinv⟩ := h.abs
        have hinv₁ := markSecond_inv (t := t) hinv
        obtain ⟨hasc, hbnd, hchars, herrs⟩ :=
          markSecond_marks (targets := targets) h.ascending h.bounded ht
        have hne := markSecond_ne_nil (r := r) (targets := targets) (t := t)
        have hkeys₁ := markSecond_keys (r := r) (targets := targets) (t := t)
        have hinc₁ := markSecond_incorrect (r := r) (targets := targets) (t := t)
        have hend₁ := markSecond_endedOnSpace (r := r) (targets := targets) (t := t)
        have hfr := step_frame (r := r.markSecond targets t) (targets := targets) (key := key)
        have hinv₂ := (step_inv hinv₁ key).1
        have hmarks :
            (((r.markSecond targets t).step targets key).1.seconds.map (·.second)).Pairwise
                (· < ·) ∧
              (∀ m ∈ ((r.markSecond targets t).step targets key).1.seconds,
                m.second ≤ secondOf t) ∧
              sumChars ((r.markSecond targets t).step targets key).1.seconds =
                ((r.markSecond targets t).step targets key).1.keys ∧
              sumErrors ((r.markSecond targets t).step targets key).1.seconds =
                ((r.markSecond targets t).step targets key).1.incorrectKeystrokes := by
          rcases hfr.marks with ⟨hk, hi, hs⟩ | ⟨ok, hk, hi, hs⟩
          · rw [hs, hk, hi]
            exact ⟨hasc, hbnd, by rw [hchars, h.chars, hkeys₁], by rw [herrs, h.errors, hinc₁]⟩
          · rw [hs, hk, hi]
            refine ⟨by rw [seconds_record]; exact hasc, ?_, ?_, ?_⟩
            · intro m hm
              have : m.second ∈ (updateLast (recordMark ok) (r.markSecond targets t).seconds).map
                  (·.second) := List.mem_map_of_mem hm
              rw [seconds_record] at this
              obtain ⟨m', hm', he⟩ := List.mem_map.1 this
              rw [← he]; exact hbnd m' hm'
            · rw [sumChars_record hne, hchars, h.chars, hkeys₁]
            · rw [sumErrors_record hne, herrs, h.errors, hinc₁]
        obtain ⟨hasc₂, hbnd₂, hchars₂, herrs₂⟩ := hmarks
        split at hok
        · next hf =>
          split at hok
          · exact absurd hok (by simp)
          · simp only [Except.ok.injEq, Prod.mk.injEq] at hok
            obtain ⟨rfl, rfl, rfl⟩ := hok
            exact {
              abs := ⟨_, _, hinv₂.congr rfl rfl rfl rfl rfl rfl⟩
              ascending := hasc₂
              bounded := hbnd₂
              chars := hchars₂
              errors := herrs₂
              ended := fun h => by cases h
              final := by
                simp only [finalChars, keys]
                by_cases hs : key = ' '
                · obtain ⟨ht', hk'⟩ := step_finished_space hs hf
                  have := hinv₁.keys
                  subst hs
                  simp only [keys] at this hk'
                  simp only [beq_self_eq_true, ite_true, ht']
                  omega
                · have := hinv₂.keys
                  simp only [keys] at this
                  simp [hs]
                  exact this
              finishedKeys := fun _ => charKeys_ge_of_finished hinv₁ hf }
        · next hf =>
          simp only [Except.ok.injEq, Prod.mk.injEq] at hok
          obtain ⟨rfl, rfl, rfl⟩ := hok
          exact {
            abs := ⟨_, _, hinv₂⟩
            ascending := hasc₂
            bounded := hbnd₂
            chars := hchars₂
            errors := herrs₂
            ended := fun _ => by
              rw [hfr.endedOnSpace, hend₁]
              exact h.ended (by simpa using hfin)
            final := by
              have := hinv₂.keys
              simp only [finalChars, hfr.endedOnSpace, hend₁, h.ended (by simpa using hfin)]
              simpa using this
            finishedKeys := fun h => by cases h }

/-- The loop keeps the invariant from any state it accepts. -/
theorem runLoop_inv (h : LoopInv targets r previousMs fin)
    (hok : runLoop limit targets log (r, previousMs, fin) = .ok (r', previousMs', fin')) :
    LoopInv targets r' previousMs' fin' := by
  induction log generalizing r previousMs fin with
  | nil =>
    simp only [runLoop, Except.ok.injEq, Prod.mk.injEq] at hok
    obtain ⟨rfl, rfl, rfl⟩ := hok
    exact h
  | cons kt rest ih =>
    obtain ⟨key, t⟩ := kt
    simp only [runLoop] at hok
    split at hok
    · exact absurd hok (by simp)
    · next st hst =>
      obtain ⟨r₁, p₁, f₁⟩ := st
      exact ih (loopStep_inv h hst) hok

/-- What holds of every replay `Replay::run` accepts: the representation invariant, strictly
ascending marks whose `chars` sum to the recorded keys and whose `errors` sum to the wrong
ones, the final text no longer than the recorded keys, and the keystroke floor of a finished
test. -/
theorem run_ok (hn : targets ≠ []) (hok : run mode targets log timesMs = .ok r) :
    (∃ committed current, Inv targets committed current r) ∧
      (r.seconds.map (·.second)).Pairwise (· < ·) ∧
      sumChars r.seconds = r.keys ∧
      sumErrors r.seconds = r.incorrectKeystrokes ∧
      r.finalChars ≤ r.keys ∧
      (r.finished = true → minKeystrokesToFinish targets.length ≤ r.keys) := by
  simp only [run] at hok
  split at hok
  · exact absurd hok (by simp)
  · next r₀ previousMs fin hst =>
    have hinv := runLoop_inv (init_loopInv hn) hst
    split at hok
    · simp only [Except.ok.injEq] at hok
      subst hok
      obtain ⟨committed, current, hi⟩ := hinv.abs
      exact ⟨⟨committed, current, hi.congr rfl rfl rfl rfl rfl rfl⟩, hinv.ascending,
        hinv.chars, hinv.errors, hinv.final, hinv.finishedKeys⟩
    · split at hok
      · simp only [Except.ok.injEq] at hok
        subst hok
        obtain ⟨committed, current, hi⟩ := hinv.abs
        exact ⟨⟨committed, current, hi.congr rfl rfl rfl rfl rfl rfl⟩, hinv.ascending,
          hinv.chars, hinv.errors, hinv.final, hinv.finishedKeys⟩
      · exact absurd hok (by simp)

/-- `committed_words` pairs exactly the committed words with their targets, plus the current
word when the test finished. -/
theorem committedWords_eq (h : Inv targets committed current r) :
    r.committedWords targets =
      committed.zip targets ++
        if r.finished then [(current, (targets[committed.length]?).getD [])] else [] := by
  have hw : ∀ w ∈ committed, ' ' ∉ w := fun w hw => (h.words w hw).2
  simp [committedWords, h.typed, h.wordStart, splitTerminator_joinSp hw,
    h.currentWord, h.target]

/-- `correct_chars` is the credit of the committed words plus the current word's length when it
is credited: its full match after a finish, else its being a prefix of the target. -/
theorem correctChars_eq (h : Inv targets committed current r) :
    r.correctChars targets =
      creditFrom targets 0 committed +
        if (if r.finished then current == (targets[committed.length]?).getD []
            else current.isPrefixOf ((targets[committed.length]?).getD []))
        then current.length else 0 := by
  simp [correctChars, h.currentWord, h.target, h.credit]

end Replay

end Platen
