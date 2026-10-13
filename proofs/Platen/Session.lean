import Platen.Replay

/-!
# The client session (`web/src/session.rs`)

The client keeps what the learner has typed as `inputs`, one entry per word with the last
entry the word in progress, and logs the keys it sends to the server. Its module doc says it
"mirrors the server's replay rules ... so the machine shows exactly what the server will
score". The refinement theorems here make that exact: replaying the client's logged keys
through the server's key-processing core (`Replay.step`) reproduces the client's words.

Left out: the line layout (`line_starts`, `cells`, `cursor_col`, `layout`), which only draws
the state. Abstractions as in `Platen.Replay`: `Nat` for `usize` and `u32`, `List Char` for
`String`, and `words` as a parameter rather than a field.

Timing is not modelled: `Replay::run` adds on top of `step` the per-second marks and the time
checks, and the client stamps each logged key with its monotone clock, so `TimesNotMonotonic`
cannot arise from the client and the marks are `Platen.Replay.run_ok`'s business.
-/

namespace Platen

/-- `Stroke`. -/
inductive Stroke
  | char (c : Char)
  | back
  deriving DecidableEq, Repr

/-- The key code the session logs for a stroke, in the server's format. -/
def Stroke.key : Stroke → Char
  | .char c => c
  | .back => backspaceKey

/-- The UI maps the backspace key to `Stroke.back`; the server's backspace code never arrives
as a character (`main.rs` builds `Stroke::Char` only from a one-character browser key value). -/
def Stroke.wellFormed (stroke : Stroke) : Prop := stroke ≠ .char backspaceKey

/-- `Effect`. -/
inductive Effect
  | ignored | struck | spaced | backed | finished
  deriving DecidableEq, Repr

/-- `Session`, minus the layout fields. -/
structure Session where
  inputs : List (List Char)
  finishesOnLastWord : Bool
  finished : Bool
  keys : List Char
  timesMs : List Nat
  deriving Repr

namespace Session

/-- `Session::new`, minus the layout. -/
def new (finishesOnLastWord : Bool) : Session :=
  { inputs := [[]], finishesOnLastWord, finished := false, keys := [], timesMs := [] }

/-- `Session::word_index`. `inputs` is never empty (`new` starts it with one word and
`backspace` pops only past a committed word), so the subtraction does not underflow. -/
def wordIndex (s : Session) : Nat := s.inputs.length - 1

/-- `Session::current`. -/
def current (s : Session) : List Char := (s.inputs[s.wordIndex]?).getD []

/-- `Session::target`. -/
def target (words : List (List Char)) (s : Session) : List Char :=
  (words[s.wordIndex]?).getD []

/-- `Session::is_last_word`. -/
def isLastWord (words : List (List Char)) (s : Session) : Bool :=
  s.wordIndex + 1 == words.length

/-- `Session::character`. -/
def character (words : List (List Char)) (s : Session) (c : Char) : Session × Effect :=
  let s := { s with inputs := updateLast (· ++ [c]) s.inputs }
  if s.finishesOnLastWord && s.isLastWord words && s.current == s.target words then
    ({ s with finished := true }, .finished)
  else (s, .struck)

/-- `Session::space`. -/
def space (words : List (List Char)) (s : Session) : Session × Effect :=
  if s.current.isEmpty then (s, .ignored)
  else if s.isLastWord words then ({ s with finished := true }, .finished)
  else ({ s with inputs := s.inputs ++ [[]] }, .spaced)

/-- `Session::backspace`. `inputs[index].pop().is_some()` is the current word being non-empty. -/
def backspace (words : List (List Char)) (s : Session) : Session × Effect :=
  if s.current ≠ [] then ({ s with inputs := updateLast List.dropLast s.inputs }, .backed)
  else if s.wordIndex = 0 ∨
      (s.inputs[s.wordIndex - 1]?).getD [] = (words[s.wordIndex - 1]?).getD [] then
    (s, .ignored)
  else ({ s with inputs := s.inputs.dropLast }, .backed)

/-- `Session::press`. -/
def press (words : List (List Char)) (s : Session) (stroke : Stroke) (timeMs : Nat) :
    Session × Effect :=
  if s.finished then (s, .ignored)
  else
    let next :=
      match stroke with
      | .back => s.backspace words
      | .char c => if c = ' ' then s.space words else s.character words c
    if next.2 ≠ .ignored ∨ stroke ≠ .char ' ' then
      ({ next.1 with keys := next.1.keys ++ [stroke.key], timesMs := next.1.timesMs ++ [timeMs] },
        next.2)
    else next

/-- `Session::finish`. -/
def finish (s : Session) : Session := { s with finished := true }

/-- `Session::words_done`. -/
def wordsDone (s : Session) : Nat := s.wordIndex + if s.finished then 1 else 0

/-- Presses every stroke in turn. -/
def pressFrom (words : List (List Char)) (s : Session) : List (Stroke × Nat) → Session
  | [] => s
  | (stroke, timeMs) :: rest => pressFrom words (s.press words stroke timeMs).1 rest

/-- Presses every stroke in turn, from a fresh session. -/
def pressAll (words : List (List Char)) (finishesOnLastWord : Bool)
    (strokes : List (Stroke × Nat)) : Session :=
  pressFrom words (new finishesOnLastWord) strokes

/-! ## Conformance checks

Mirrors of `session.rs` tests. -/

/-- Presses the characters of `text` at time 0, backspace for `\x08`, returning the effects. -/
def typeAll (words : List (List Char)) (s : Session) (text : String) : Session × List Effect :=
  text.toList.foldl
    (fun (s, effects) c =>
      let stroke := if c = backspaceKey then Stroke.back else Stroke.char c
      let (s, effect) := s.press words stroke 0
      (s, effects ++ [effect]))
    (s, [])

/-! `logs_keys_in_server_format_and_skips_leading_space`. -/
#guard
  let (s, _) := typeAll [['a', 'b'], ['c', 'd']] (new true) " ax\x08b c"
  s.keys == "ax\x08b c".toList && s.timesMs.length == 6

/-! `finishes_word_test_on_last_word`. -/
#guard
  let (s, effects) := typeAll [['a', 'b'], ['c', 'd']] (new true) "ab cd"
  effects.getLast? == some .finished && (s.press [['a', 'b'], ['c', 'd']] (.char 'x') 0).2 == .ignored

/-! `backspace_cannot_reopen_a_correct_word`. -/
#guard
  let words := [['a', 'b'], ['c', 'd'], ['e', 'f']]
  let (s, _) := typeAll words (new true) "ab "
  let (s, back₁) := s.press words .back 0
  let (s, _) := typeAll words s "cx "
  let (_, back₂) := s.press words .back 0
  back₁ == .ignored && back₂ == .backed

/-! A time test does not end on the last word typed exactly; the server would call it
`TooFast` instead. -/
#guard
  let (s, effects) := typeAll [['a', 'b'], ['c', 'd']] (new false) "ab cd"
  effects.getLast? == some .struck && !s.finished && s.inputs == [['a', 'b'], ['c', 'd']]

/-! ## The server's key-processing core -/

/-- `Replay.step` over a key log, with `ExtraInput` as `none`: the part of `Replay::run` the
client mirrors. -/
def replayKeys (targets : List (List Char)) : List Char → Replay × Bool → Option (Replay × Bool)
  | [], st => some st
  | key :: rest, (r, fin) => if fin then none else replayKeys targets rest (r.step targets key)

theorem replayKeys_append (targets : List (List Char)) (a b : List Char) (st : Replay × Bool) :
    replayKeys targets (a ++ b) st = (replayKeys targets a st).bind (replayKeys targets b) := by
  induction a generalizing st with
  | nil => rfl
  | cons key rest ih =>
    obtain ⟨r, fin⟩ := st
    cases fin <;> simp [replayKeys, ih]

/-- A key log the server finishes on ends with the keystroke floor of
`Replay.charKeys_ge_of_finished`. -/
theorem replayKeys_finished {targets committed : List (List Char)} {current : List Char}
    {r₀ r : Replay} (h : Replay.Inv targets committed current r₀) (log : List Char)
    (hrep : replayKeys targets log (r₀, false) = some (r, true)) :
    minKeystrokesToFinish targets.length ≤ r.keys := by
  induction log generalizing committed current r₀ with
  | nil => simp [replayKeys] at hrep
  | cons key rest ih =>
    simp only [replayKeys, ite_false, Bool.false_eq_true] at hrep
    cases hfin : (r₀.step targets key).2
    · rw [show r₀.step targets key = ((r₀.step targets key).1, false) from
        Prod.ext rfl hfin] at hrep
      exact ih (Replay.step_inv h key).1 hrep
    · rw [show r₀.step targets key = ((r₀.step targets key).1, true) from
        Prod.ext rfl hfin] at hrep
      cases rest with
      | nil =>
        simp only [replayKeys, Option.some.injEq, Prod.mk.injEq] at hrep
        rw [← hrep.1]
        exact Replay.charKeys_ge_of_finished h hfin
      | cons _ _ => simp [replayKeys] at hrep

/-! ## One stroke on the client, against the abstract step -/

variable {words committed done : List (List Char)} {current last : List Char} {s : Session}

theorem wordIndex_eq (hin : s.inputs = committed ++ [current]) :
    s.wordIndex = committed.length := by
  simp [Session.wordIndex, hin]

theorem current_eq (hin : s.inputs = committed ++ [current]) : s.current = current := by
  simp [Session.current, wordIndex_eq hin, hin]

theorem target_eq (hin : s.inputs = committed ++ [current]) :
    s.target words = (words[committed.length]?).getD [] := by
  simp [Session.target, wordIndex_eq hin]

theorem isLastWord_eq (hin : s.inputs = committed ++ [current]) :
    s.isLastWord words = (committed.length + 1 == words.length) := by
  simp [Session.isLastWord, wordIndex_eq hin]

/-- The previous word as `backspace` indexes it. -/
theorem previous_eq (hin : s.inputs = done ++ [last] ++ [current]) :
    (s.inputs[s.wordIndex - 1]?).getD [] = last := by
  rw [wordIndex_eq hin, hin]
  simp

/-- `character` on an unfinished session: the letter joins the current word, and a word test
finishes when that completes the last word exactly. -/
theorem character_spec (hin : s.inputs = committed ++ [current]) (hfin : s.finished = false)
    (c : Char) :
    (s.character words c).1.inputs = committed ++ [current ++ [c]] ∧
      (s.character words c).1.keys = s.keys ∧
      (s.character words c).1.finishesOnLastWord = s.finishesOnLastWord ∧
      (s.character words c).1.finished =
        (s.finishesOnLastWord && (committed.length + 1 == words.length) &&
          (current ++ [c] == (words[committed.length]?).getD [])) := by
  have hin' : ({ s with inputs := updateLast (· ++ [c]) s.inputs } : Session).inputs =
      committed ++ [current ++ [c]] := by
    simp [hin, Replay.updateLast_concat]
  unfold character
  dsimp only
  rw [isLastWord_eq hin', current_eq hin', target_eq hin']
  split
  · next h => simp [hin, Replay.updateLast_concat, h]
  · next h => simp [hin, Replay.updateLast_concat, hfin, h]

/-- `space` on the three shapes of state: before any letter it is ignored, on the last word it
ends the test, elsewhere it opens the next word. -/
theorem space_ignored (hin : s.inputs = committed ++ [current]) (hne : current = []) :
    s.space words = (s, .ignored) := by
  have h1 : s.current.isEmpty = true := by simp [current_eq hin, hne]
  unfold space
  rw [ite_eq_left h1]

theorem space_last (hin : s.inputs = committed ++ [current]) (hne : current ≠ [])
    (hl : committed.length + 1 = words.length) :
    s.space words = ({ s with finished := true }, .finished) := by
  have h1 : ¬ s.current.isEmpty = true := by simp [current_eq hin, hne]
  have h2 : s.isLastWord words = true := by simp [isLastWord_eq hin, hl]
  unfold space
  rw [ite_eq_right h1, ite_eq_left h2]

theorem space_commit (hin : s.inputs = committed ++ [current]) (hne : current ≠ [])
    (hl : committed.length + 1 ≠ words.length) :
    s.space words = ({ s with inputs := s.inputs ++ [[]] }, .spaced) := by
  have h1 : ¬ s.current.isEmpty = true := by simp [current_eq hin, hne]
  have h2 : ¬ s.isLastWord words = true := by simp [isLastWord_eq hin, hl]
  unfold space
  rw [ite_eq_right h1, ite_eq_right h2]

/-- What one stroke does on an unfinished session, in terms of `Replay.absStep`. Either it is a
space before any letter, which the client neither applies nor logs, or it is logged and moves the
words exactly as the server's `absStep` does. The client finishes when the server does, except
that a time test's client does not finish on a character. -/
theorem press_step (hin : s.inputs = committed ++ [current]) (hfin : s.finished = false)
    {stroke : Stroke} (hc : stroke.wellFormed) (timeMs : Nat) :
    ((s.press words stroke timeMs).1 = s ∧ stroke = .char ' ' ∧ current = []) ∨
      ((s.press words stroke timeMs).1.keys = s.keys ++ [stroke.key] ∧
        (s.press words stroke timeMs).1.inputs =
          (Replay.absStep words committed current stroke.key).1.1 ++
            [(Replay.absStep words committed current stroke.key).1.2] ∧
        (s.press words stroke timeMs).1.finished =
          ((Replay.absStep words committed current stroke.key).2 &&
            (s.finishesOnLastWord || stroke.key == ' ')) ∧
        (s.press words stroke timeMs).1.finishesOnLastWord = s.finishesOnLastWord) := by
  have hcur := current_eq hin
  have hidx := wordIndex_eq hin
  have hlast := isLastWord_eq (words := words) hin
  unfold press
  rw [ite_eq_right (by simp [hfin])]
  cases stroke with
  | back =>
    simp only [Stroke.key]
    rw [ite_eq_left (Or.inr (by simp))]
    right
    unfold backspace Replay.absStep
    rw [ite_eq_left rfl]
    by_cases hne : current = []
    · have h1 : ¬ (s.current ≠ []) := fun h => h (hcur ▸ hne)
      have h2 : ¬ (current ≠ []) := fun h => h hne
      rw [ite_eq_right h1, ite_eq_right h2]
      subst hne
      rcases List.eq_nil_or_concat committed with rfl | ⟨done, last, rfl⟩
      · have h3 : s.wordIndex = 0 ∨
            (s.inputs[s.wordIndex - 1]?).getD [] = (words[s.wordIndex - 1]?).getD [] :=
          Or.inl (by simp [hidx])
        rw [ite_eq_left h3]
        simp [hin, hfin]
      · simp only [List.concat_eq_append] at hin hidx ⊢
        rw [previous_eq hin]
        simp only [hidx, List.length_append, List.length_singleton, Nat.add_sub_cancel,
          Nat.add_one_ne_zero, false_or, List.getLast?_concat]
        by_cases hl : last = (words[done.length]?).getD []
        · rw [ite_eq_left hl, ite_eq_left hl]
          simp [hin, hfin]
        · rw [ite_eq_right hl, ite_eq_right hl]
          simp [hin, hfin]
    · have h1 : s.current ≠ [] := hcur ▸ hne
      rw [ite_eq_left h1, ite_eq_left hne]
      simp [hin, Replay.updateLast_concat, hfin]
  | char c =>
    simp only [Stroke.key]
    by_cases hs : c = ' '
    · subst hs
      by_cases hne : current = []
      · left
        simp [space_ignored hin hne, hne]
      · right
        by_cases hl : committed.length + 1 = words.length
        · simp [space_last hin hne hl, Replay.absStep, Replay.space_ne_backspaceKey, hne, hl, hin]
        · simp [space_commit hin hne hl, Replay.absStep, Replay.space_ne_backspaceKey, hne, hl,
            hin, hfin]
    · have hb : c ≠ backspaceKey := fun h => hc (by rw [h])
      simp only [hs, ite_false, Stroke.char.injEq, ne_eq, not_false_eq_true, or_true, ite_true]
      right
      unfold Replay.absStep
      rw [ite_eq_right hb, ite_eq_right hs]
      obtain ⟨hi, hk, hf, hfin'⟩ := character_spec (words := words) hin hfin c
      have hcs : (c == ' ') = false := by simpa using hs
      refine ⟨by simp [hk], by simp [hi], ?_, by simp [hf]⟩
      simp only [hfin', hcs, Bool.or_false]
      cases s.finishesOnLastWord <;> cases (committed.length + 1 == words.length) <;>
        cases (current ++ [c] == (words[committed.length]?).getD []) <;> rfl

/-! ## Word tests: the client shows exactly what the server scores -/

/-- The simulation invariant of a word test: the client's words are the server's abstract
state after replaying the logged keys, and both agree on whether the test is over. -/
structure WordSim (words : List (List Char)) (s : Session) : Prop where
  fol : s.finishesOnLastWord = true
  abs : ∃ committed current r, s.inputs = committed ++ [current] ∧
    replayKeys words s.keys (Replay.init, false) = some (r, s.finished) ∧
    Replay.Inv words committed current r

theorem WordSim.new (hw : words ≠ []) : WordSim words (new true) :=
  ⟨rfl, [], [], Replay.init, rfl, rfl, Replay.init_inv hw⟩

theorem WordSim.press (h : WordSim words s) {stroke : Stroke} (hc : stroke.wellFormed)
    (timeMs : Nat) : WordSim words (s.press words stroke timeMs).1 := by
  obtain ⟨committed, current, r, hin, hrep, hinv⟩ := h.abs
  by_cases hfin : s.finished = true
  · unfold Session.press; rw [ite_eq_left hfin]; exact h
  · have hfin' : s.finished = false := by simpa using hfin
    rcases press_step (words := words) hin hfin' hc timeMs with ⟨he, -, -⟩ | ⟨hk, hi, hf, hfol⟩
    · rw [he]; exact h
    · refine ⟨by rw [hfol]; exact h.fol, _, _, (r.step words stroke.key).1, hi, ?_,
        (Replay.step_inv hinv stroke.key).1⟩
      rw [hk, replayKeys_append, hrep, hfin', hf, h.fol, ← (Replay.step_inv hinv stroke.key).2]
      simp [replayKeys]

theorem WordSim.pressFrom (h : WordSim words s) (strokes : List (Stroke × Nat))
    (hc : ∀ x ∈ strokes, x.1.wellFormed) : WordSim words (pressFrom words s strokes) := by
  induction strokes generalizing s with
  | nil => exact h
  | cons x rest ih =>
    obtain ⟨stroke, timeMs⟩ := x
    exact ih (h.press (hc (stroke, timeMs) (by simp)) timeMs) fun y hy => hc y (by simp [hy])

/-- Word test refinement: replaying the client's logged keys through the server's core never
hits `ExtraInput`, ends in the client's words, and finishes exactly when the client does. -/
theorem word_test_refines (hw : words ≠ []) (strokes : List (Stroke × Nat))
    (hc : ∀ x ∈ strokes, x.1.wellFormed) :
    ∃ committed current r, (pressAll words true strokes).inputs = committed ++ [current] ∧
      replayKeys words (pressAll words true strokes).keys (Replay.init, false) =
        some (r, (pressAll words true strokes).finished) ∧
      Replay.Inv words committed current r :=
  ((WordSim.new hw).pressFrom strokes hc).abs

/-! ## Time tests: agreement until the server finishes -/

/-- The simulation invariant of a time test: whenever the server's replay of the logged keys has
not finished, the client has not either and shows the server's words. -/
structure TimeSim (words : List (List Char)) (s : Session) : Prop where
  fol : s.finishesOnLastWord = false
  abs : ∃ committed current, s.inputs = committed ++ [current] ∧
    ∀ r, replayKeys words s.keys (Replay.init, false) = some (r, false) →
      s.finished = false ∧ Replay.Inv words committed current r

theorem TimeSim.new (hw : words ≠ []) : TimeSim words (new false) :=
  ⟨rfl, [], [], rfl, fun _ hr =>
    ⟨rfl, (Prod.mk.inj (Option.some.inj hr)).1 ▸ Replay.init_inv hw⟩⟩

theorem TimeSim.press (h : TimeSim words s) {stroke : Stroke} (hc : stroke.wellFormed)
    (timeMs : Nat) : TimeSim words (s.press words stroke timeMs).1 := by
  obtain ⟨committed, current, hin, hrep⟩ := h.abs
  by_cases hfin : s.finished = true
  · unfold Session.press; rw [ite_eq_left hfin]; exact h
  · have hfin' : s.finished = false := by simpa using hfin
    rcases press_step (words := words) hin hfin' hc timeMs with ⟨he, -, -⟩ | ⟨hk, hi, hf, hfol⟩
    · rw [he]; exact h
    · refine ⟨by rw [hfol]; exact h.fol, _, _, hi, fun r' hr' => ?_⟩
      rw [hk, replayKeys_append, Option.bind_eq_some_iff] at hr'
      obtain ⟨⟨r, fin⟩, hr, hstep⟩ := hr'
      cases fin
      · obtain ⟨-, hinv⟩ := hrep r hr
        simp only [replayKeys, Bool.false_eq_true, ite_false, Option.some.injEq] at hstep
        have hsnd : (r.step words stroke.key).2 = false := by rw [hstep]
        refine ⟨?_, ?_⟩
        · rw [hf, ← (Replay.step_inv hinv stroke.key).2, hsnd]; rfl
        · have := (Replay.step_inv hinv stroke.key).1
          rw [hstep] at this
          exact this
      · simp [replayKeys] at hstep

theorem TimeSim.pressFrom (h : TimeSim words s) (strokes : List (Stroke × Nat))
    (hc : ∀ x ∈ strokes, x.1.wellFormed) : TimeSim words (pressFrom words s strokes) := by
  induction strokes generalizing s with
  | nil => exact h
  | cons x rest ih =>
    obtain ⟨stroke, timeMs⟩ := x
    exact ih (h.press (hc (stroke, timeMs) (by simp)) timeMs) fun y hy => hc y (by simp [hy])

/-- Time test refinement: as long as the server's replay of the logged keys has not finished,
the client has not finished and shows the server's words. -/
theorem time_test_refines (hw : words ≠ []) (strokes : List (Stroke × Nat))
    (hc : ∀ x ∈ strokes, x.1.wellFormed) {r : Replay}
    (hr : replayKeys words (pressAll words false strokes).keys (Replay.init, false) =
      some (r, false)) :
    (pressAll words false strokes).finished = false ∧
      ∃ committed current, (pressAll words false strokes).inputs = committed ++ [current] ∧
        Replay.Inv words committed current r := by
  obtain ⟨committed, current, hin, hrep⟩ := ((TimeSim.new hw).pressFrom strokes hc).abs
  obtain ⟨hfin, hinv⟩ := hrep r hr
  exact ⟨hfin, committed, current, hin, hinv⟩

/-- The only divergence: when the server's replay finishes a time test, the logged keys reach
the keystroke floor of the text, and with `Platen.time_text_outlasts_cap` that is above the
speed cap. The client keeps typing; the server answers `TooFast`. -/
theorem time_test_diverges (hw : words ≠ []) (strokes : List (Stroke × Nat)) {r : Replay}
    (hr : replayKeys words (pressAll words false strokes).keys (Replay.init, false) =
      some (r, true)) :
    minKeystrokesToFinish words.length ≤ r.keys :=
  replayKeys_finished (Replay.init_inv hw) _ hr

theorem time_test_tooFast (m : TestMode) {limitMs : Nat} (hm : m.timeLimitMs = some limitMs)
    (hn : words.length = m.wordCount) (hw : words ≠ []) (strokes : List (Stroke × Nat))
    {r : Replay}
    (hr : replayKeys words (pressAll words false strokes).keys (Replay.init, false) =
      some (r, true)) :
    ¬ underCap r.keys limitMs := by
  intro hcap
  have := time_text_outlasts_cap m hm hcap
  have := time_test_diverges hw strokes hr
  rw [hn] at this
  omega

end Session

end Platen
