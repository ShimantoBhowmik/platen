/-!
# Sheet line wrapping (`web/src/session.rs`)

`Session::new` wraps the test's words into lines of at most `cols` cells, and `Session::line_of`
finds the line a word is on.

Abstractions: each word is its length (`word.chars().count()`), a `Nat`; `usize` is `Nat`, and
`width + 1 + len` cannot overflow for words that fit in memory. `slice::partition_point` is a
binary search whose answer is only specified when the predicate holds on a prefix of the slice
and fails on the rest; `start <= word` is such a predicate on the strictly increasing
`line_starts`, so the model takes the length of that prefix.

The words are nonempty, as the server's word list is. A zero-length word leaves `width` at 0,
so the next word is placed without the space between them being counted.
-/

namespace Platen

/-- The loop of `Session::new` from word `index` on, with `width` the cells used on the current
line: the line starts it pushes. -/
def wrapFrom (lens : List Nat) (cols : Nat) (index width : Nat) : List Nat :=
  if h : index < lens.length then
    let len := lens[index]
    let needed := if width = 0 then len else width + 1 + len
    if width > 0 ∧ needed > cols then index :: wrapFrom lens cols (index + 1) len
    else wrapFrom lens cols (index + 1) needed
  else []
termination_by lens.length - index

/-- `line_starts` as `Session::new` lays it out. -/
def lineStarts (lens : List Nat) (cols : Nat) : List Nat := 0 :: wrapFrom lens cols 0 0

/-- `slice::partition_point` on a slice partitioned by `p`. -/
def partitionPoint (p : Nat → Bool) (xs : List Nat) : Nat := (xs.takeWhile p).length

/-- `Session::line_of`. -/
def lineOf (starts : List Nat) (word : Nat) : Nat :=
  partitionPoint (fun start => decide (start ≤ word)) starts - 1

/-- The cells words `s..e` take on one line: their lengths and a space between each two. -/
def lineWidth (lens : List Nat) (s e : Nat) : Nat :=
  ((lens.take e).drop s).sum + (e - s - 1)

/-- What every line `s..e` of a layout satisfies, for `n` words: it holds at least one word,
fits in `cols` unless it is a single word, and (unless it is the last) the next line's first
word would not have fit after it. -/
def GoodLine (lens : List Nat) (cols : Nat) (s e : Nat) : Prop :=
  s < e ∧ (e = s + 1 ∨ lineWidth lens s e ≤ cols) ∧
    ∀ h : e < lens.length, cols < lineWidth lens s e + 1 + lens[e]

/-- Every line of `starts`, each running to the next start and the last to `n`, is good. -/
def AllLines (good : Nat → Nat → Prop) (n : Nat) : List Nat → Prop
  | [] => True
  | [s] => good s n
  | s :: e :: rest => good s e ∧ AllLines good n (e :: rest)

theorem AllLines.get {good : Nat → Nat → Prop} {n : Nat} :
    ∀ {starts : List Nat}, AllLines good n starts → ∀ l (h : l < starts.length),
      good starts[l] (starts[l + 1]?.getD n)
  | [], _, _, h => absurd h (Nat.not_lt_zero _)
  | [s], hs, 0, _ => hs
  | s :: e :: rest, hs, 0, _ => hs.1
  | s :: e :: rest, hs, l + 1, h => AllLines.get hs.2 l (by simp at h ⊢; omega)

theorem lineWidth_sum_succ {lens : List Nat} {s i : Nat} (hs : s ≤ i) (hi : i < lens.length) :
    ((lens.take (i + 1)).drop s).sum = ((lens.take i).drop s).sum + lens[i] := by
  rw [List.take_add_one, List.drop_append_of_le_length (by simp; omega)]
  simp [List.getElem?_eq_getElem hi]

/-- A one-word line is as wide as the word. -/
theorem lineWidth_single {lens : List Nat} {i : Nat} (hi : i < lens.length) :
    lineWidth lens i (i + 1) = lens[i] := by
  have := lineWidth_sum_succ (Nat.le_refl i) hi
  simp at this; simp [lineWidth, this]

/-- Adding a word to a line adds a space and the word. -/
theorem lineWidth_extend {lens : List Nat} {s i : Nat} (hs : s < i) (hi : i < lens.length) :
    lineWidth lens s (i + 1) = lineWidth lens s i + 1 + lens[i] := by
  have := lineWidth_sum_succ (Nat.le_of_lt hs) hi
  simp only [lineWidth, this]; omega

theorem lineWidth_pos {lens : List Nat} (hpos : ∀ x ∈ lens, 0 < x) {s e : Nat} (hse : s < e)
    (he : e ≤ lens.length) : 0 < lineWidth lens s e := by
  by_cases h : e = s + 1
  · subst h; rw [lineWidth_single (by omega)]; exact hpos _ (List.getElem_mem _)
  · unfold lineWidth; omega

/-- The loop invariant: the current line starts at `cur`, and `width` is its width so far. -/
structure WrapInv (lens : List Nat) (cols cur index width : Nat) : Prop where
  cur_le : cur ≤ index
  cur_lt : cur < lens.length
  index_le : index ≤ lens.length
  width_eq : width = if index = cur then 0 else lineWidth lens cur index
  fits : index ≤ cur + 1 ∨ lineWidth lens cur index ≤ cols

theorem wrapFrom_lines {lens : List Nat} {cols : Nat} (hpos : ∀ x ∈ lens, 0 < x) :
    ∀ index cur width, WrapInv lens cols cur index width →
      AllLines (GoodLine lens cols) lens.length (cur :: wrapFrom lens cols index width) := by
  intro index
  induction index using (measure fun index => lens.length - index).wf.induction with
  | h index ih =>
  intro cur width ⟨hle, hcur, hidx, hwidth, hfits⟩
  rw [wrapFrom]
  split
  · rename_i hi
    have next : lens.length - (index + 1) < lens.length - index := by omega
    by_cases hstart : index = cur
    · -- The line's first word: `width` is 0, so it never wraps.
      subst hstart
      simp only [ite_true] at hwidth
      simp only [hwidth, Nat.lt_irrefl, false_and, ite_false, ite_true]
      refine ih (index + 1) next index _ ⟨by omega, hcur, by omega, ?_, by omega⟩
      simp [lineWidth_single hi]
    · have hlt : cur < index := by omega
      simp only [hstart, ite_false] at hwidth
      have hwpos : 0 < width := hwidth ▸ lineWidth_pos hpos hlt hidx
      have hext := lineWidth_extend hlt hi
      simp only [Nat.ne_of_gt hwpos, ite_false]
      split
      · rename_i hwrap
        refine ⟨⟨hlt, ?_, fun _ => by omega⟩, ?_⟩
        · rcases hfits with h | h
          · left; omega
          · right; exact h
        · refine ih (index + 1) next index _ ⟨by omega, hi, by omega, ?_, by omega⟩
          simp [lineWidth_single hi]
      · refine ih (index + 1) next cur _ ⟨by omega, hcur, by omega, ?_, ?_⟩
        · simp [show index + 1 ≠ cur by omega]; omega
        · right; omega
  · rename_i hi
    have hn : index = lens.length := by omega
    subst hn
    exact ⟨hcur, by omega, fun h => by omega⟩

/-- The layout's lines are all good. -/
theorem lineStarts_lines {lens : List Nat} (cols : Nat) (hpos : ∀ x ∈ lens, 0 < x)
    (hn : 0 < lens.length) :
    AllLines (GoodLine lens cols) lens.length (lineStarts lens cols) :=
  wrapFrom_lines hpos 0 0 0 ⟨Nat.le_refl 0, hn, Nat.zero_le _, by simp, by omega⟩

theorem AllLines.lt_of_cons {lens : List Nat} {cols n : Nat} :
    ∀ {s : Nat} {rest : List Nat}, AllLines (GoodLine lens cols) n (s :: rest) →
      s < n ∧ ∀ x ∈ rest, s < x
  | s, [], h => ⟨h.1, by simp⟩
  | s, e :: rest, h =>
    have ih := AllLines.lt_of_cons h.2
    ⟨by have := h.1.1; omega, by
      intro x hx
      rcases List.mem_cons.mp hx with rfl | hx
      · exact h.1.1
      · have := ih.2 x hx; have := h.1.1; omega⟩

theorem AllLines.pairwise {lens : List Nat} {cols n : Nat} :
    ∀ {starts : List Nat}, AllLines (GoodLine lens cols) n starts →
      starts.Pairwise (· < ·) ∧ ∀ x ∈ starts, x < n
  | [], _ => ⟨List.Pairwise.nil, by simp⟩
  | [s], h => ⟨List.pairwise_singleton _ s, by simpa using h.1⟩
  | s :: e :: rest, h =>
    have ih := AllLines.pairwise h.2
    have hs := AllLines.lt_of_cons h
    ⟨List.pairwise_cons.mpr ⟨hs.2, ih.1⟩, by
      intro x hx
      rcases List.mem_cons.mp hx with rfl | hx
      · exact hs.1
      · exact ih.2 x hx⟩

/-- The first line starts at the first word. -/
theorem lineStarts_head (lens : List Nat) (cols : Nat) : (lineStarts lens cols).head? = some 0 :=
  rfl

/-- The line starts strictly increase. -/
theorem lineStarts_sorted {lens : List Nat} (cols : Nat) (hpos : ∀ x ∈ lens, 0 < x)
    (hn : 0 < lens.length) : (lineStarts lens cols).Pairwise (· < ·) :=
  (lineStarts_lines cols hpos hn).pairwise.1

/-- Every line starts at a word of the text. -/
theorem lineStarts_lt {lens : List Nat} (cols : Nat) (hpos : ∀ x ∈ lens, 0 < x)
    (hn : 0 < lens.length) : ∀ x ∈ lineStarts lens cols, x < lens.length :=
  (lineStarts_lines cols hpos hn).pairwise.2

/-- Each line fits in `cols` cells, unless it holds a single word. -/
theorem lineStarts_fit {lens : List Nat} (cols : Nat) (hpos : ∀ x ∈ lens, 0 < x)
    (hn : 0 < lens.length) (l : Nat) (hl : l < (lineStarts lens cols).length) :
    let starts := lineStarts lens cols
    let e := starts[l + 1]?.getD lens.length
    e = starts[l] + 1 ∨ lineWidth lens starts[l] e ≤ cols :=
  ((lineStarts_lines cols hpos hn).get l hl).2.1

/-- Wrapping is greedy: the next line's first word would not have fit on this line. -/
theorem lineStarts_greedy {lens : List Nat} (cols : Nat) (hpos : ∀ x ∈ lens, 0 < x)
    (hn : 0 < lens.length) (l : Nat) (hl : l + 1 < (lineStarts lens cols).length) :
    let starts := lineStarts lens cols
    ∃ h : starts[l + 1] < lens.length,
      cols < lineWidth lens starts[l] starts[l + 1] + 1 + lens[starts[l + 1]] := by
  have hlt := lineStarts_lt cols hpos hn _ (List.getElem_mem hl)
  have := ((lineStarts_lines cols hpos hn).get l (by omega)).2.2
  simp only [List.getElem?_eq_getElem hl, Option.getD_some] at this
  exact ⟨hlt, this hlt⟩

theorem takeWhile_spec (p : Nat → Bool) :
    ∀ (xs : List Nat), (xs.takeWhile p).length ≤ xs.length ∧
      (∀ i, i < (xs.takeWhile p).length → xs[i]?.map p = some true) ∧
      ∀ h : (xs.takeWhile p).length < xs.length, p xs[(xs.takeWhile p).length] = false
  | [] => by simp
  | x :: xs => by
    have ih := takeWhile_spec p xs
    by_cases hx : p x
    · simp only [List.takeWhile_cons, hx, ite_true, List.length_cons]
      refine ⟨by omega, fun i h => ?_, fun h => by simpa using ih.2.2 (by omega)⟩
      cases i with
      | zero => simpa using hx
      | succ i => simpa using ih.2.1 i (by omega)
    · simp only [List.takeWhile_cons, hx]
      exact ⟨by simp, fun i h => absurd h (Nat.not_lt_zero _), fun _ => by simpa using hx⟩

/-- `line_of` never underflows, and returns the line whose words include `word`: its start is
at most `word`, and the next line's start (or the word count, after the last line) is above. -/
theorem lineOf_spec {lens : List Nat} (cols : Nat) {word : Nat} (hw : word < lens.length) :
    let starts := lineStarts lens cols
    1 ≤ partitionPoint (fun start => decide (start ≤ word)) starts ∧
      ∃ start, starts[lineOf starts word]? = some start ∧ start ≤ word ∧
        word < starts[lineOf starts word + 1]?.getD lens.length := by
  intro starts
  obtain ⟨hle, hall, hstop⟩ := takeWhile_spec (fun start => decide (start ≤ word)) starts
  have hpos : 1 ≤ partitionPoint (fun start => decide (start ≤ word)) starts := by
    simp [partitionPoint, starts, lineStarts]
  refine ⟨hpos, ?_⟩
  simp only [lineOf, partitionPoint] at hpos ⊢
  generalize hk : (starts.takeWhile fun start => decide (start ≤ word)).length = k at *
  have hprev := hall (k - 1) (by omega)
  rw [List.getElem?_eq_getElem (by omega)] at hprev ⊢
  refine ⟨_, rfl, by simpa using hprev, ?_⟩
  rw [show k - 1 + 1 = k by omega]
  by_cases hlt : k < starts.length
  · simp only [List.getElem?_eq_getElem hlt, Option.getD_some]
    simpa using hstop hlt
  · simp [List.getElem?_eq_none (by omega : starts.length ≤ k), hw]

/-- The lengths of `words`. -/
private def lensOf (words : List String) : List Nat := words.map (·.length)

-- `wraps_words_into_lines_of_the_sheet`, with the tests' 12 columns.
#guard lineStarts (lensOf ["alpha", "beta", "gamma", "delta"]) 12 = [0, 2]

-- `fills_a_line_with_as_many_short_words_as_fit`: twelve one-letter words and their spaces
-- take 23 of 24 columns.
#guard lineStarts (lensOf (List.replicate 13 "a")) 24 = [0, 12]

#guard (List.range 4).map (lineOf (lineStarts (lensOf ["alpha", "beta", "gamma", "delta"]) 12))
  = [0, 0, 1, 1]

end Platen
