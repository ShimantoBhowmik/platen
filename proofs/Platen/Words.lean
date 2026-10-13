/-!
# Test text generation (`server/src/words.rs`)

The seeded generator that picks a test's words. Words are modeled by their index in the
embedded list, and the list by its length; `generate` maps indices to words at the end.

Abstractions:
* `SplitMix64::next_u64` is transcribed bit for bit on `UInt64`, whose `+`, `*`, `>>>` and `^^^`
  wrap and shift exactly like Rust's `wrapping_add`, `wrapping_mul`, `>>` and `^`.
* Every other `usize` is a `Nat`. `below` computes `(u128(x) * u128(bound)) >> 64`; with
  `x, bound < 2^64` the product is below `2^128`, so the `u128` never wraps and the `Nat`
  formula `x * bound / 2^64` is exact. Its result is below `bound`, so `as usize` never truncates.
  The one subtraction, `bound - count` in `below_excluding`, is shown never to underflow.
* `[Option<usize>; 2]` is a pair, and `sort_unstable` on it is the (min, max) pair under Rust's
  `Option` order. Equal elements are identical values, so stability cannot matter.

`#guard` below pins the model to the Rust test `sequence_is_pinned_across_releases`.
-/

namespace Platen

/-- `SplitMix64`. -/
structure SplitMix64 where
  state : UInt64

namespace SplitMix64

/-- `SplitMix64::next_u64`: the output and the advanced generator. -/
def nextU64 (rng : SplitMix64) : UInt64 × SplitMix64 :=
  let state := rng.state + 0x9E3779B97F4A7C15
  let z := state
  let z := (z ^^^ (z >>> 30)) * 0xBF58476D1CE4E5B9
  let z := (z ^^^ (z >>> 27)) * 0x94D049BB133111EB
  (z ^^^ (z >>> 31), ⟨state⟩)

/-- The arithmetic of `SplitMix64::below` on one 64-bit draw `x`. -/
def scale (x : UInt64) (bound : Nat) : Nat := x.toNat * bound / 2 ^ 64

/-- `SplitMix64::below`. -/
def below (rng : SplitMix64) (bound : Nat) : Nat × SplitMix64 :=
  let (x, rng) := rng.nextU64
  (scale x bound, rng)

end SplitMix64

/-- The `excluded` / `recent` array `[Option<usize>; 2]`. -/
abbrev Excluded := Option Nat × Option Nat

/-- `Ord for Option<usize>`: `None` before every `Some`, and `Some`s by value. -/
def optionLe : Option Nat → Option Nat → Bool
  | none, _ => true
  | some _, none => false
  | some a, some b => decide (a ≤ b)

/-- `excluded.sort_unstable()` on the two-element array. -/
def sortExcluded (excluded : Excluded) : Excluded :=
  if optionLe excluded.1 excluded.2 then excluded else (excluded.2, excluded.1)

/-- `excluded.iter().flatten()`: the `Some` values in array order. -/
def flatten (excluded : Excluded) : List Nat := [excluded.1, excluded.2].filterMap id

/-- `allowed` in `SplitMix64::below_excluding`: `bound - excluded.iter().flatten().count()`. -/
def allowed (bound : Nat) (excluded : Excluded) : Nat :=
  bound - (flatten (sortExcluded excluded)).length

/-- The fold in `SplitMix64::below_excluding`: shifts `draw` past each excluded index, in the
sorted order. -/
def placeDraw (excluded : Excluded) (draw : Nat) : Nat :=
  (flatten (sortExcluded excluded)).foldl
    (fun index skip => index + if index ≥ skip then 1 else 0) draw

/-- `SplitMix64::below_excluding`. -/
def SplitMix64.belowExcluding (rng : SplitMix64) (bound : Nat) (excluded : Excluded) :
    Nat × SplitMix64 :=
  let (draw, rng) := rng.below (allowed bound excluded)
  (placeDraw excluded draw, rng)

/-- The contract of `below_excluding`: the `Some` entries are distinct and below `bound`. -/
def ExcludedOk (bound : Nat) : Excluded → Prop
  | (none, none) => True
  | (some a, none) => a < bound
  | (none, some a) => a < bound
  | (some a, some b) => a ≠ b ∧ a < bound ∧ b < bound

/-- Whether `i` is one of the `Some` entries. -/
def IsExcluded (excluded : Excluded) (i : Nat) : Prop :=
  excluded.1 = some i ∨ excluded.2 = some i

/-- One iteration of the loop in `generate`: the chosen index and the next `(rng, recent)`. -/
def generateStep (len : Nat) (s : SplitMix64 × Excluded) : Nat × (SplitMix64 × Excluded) :=
  let (index, rng) := s.1.belowExcluding len s.2
  (index, (rng, (s.2.2, some index)))

/-- The loop in `generate`, running `count` more iterations from state `s`. -/
def generateLoop (len : Nat) : Nat → SplitMix64 × Excluded → List Nat
  | 0, _ => []
  | count + 1, s =>
    let step := generateStep len s
    step.1 :: generateLoop len count step.2

/-- `(rng, recent)` after `k` iterations of the loop in `generate`. Not a Rust item; it names
the state each `below_excluding` call sees, to state that call's contract. -/
def generateState (len : Nat) : Nat → SplitMix64 × Excluded → SplitMix64 × Excluded
  | 0, s => s
  | k + 1, s => (generateStep len (generateState len k s)).2

/-- `generate` as list indices, for a word list of `len` entries. -/
def generateIndices (len : Nat) (seed : UInt64) (count : Nat) : List Nat :=
  generateLoop len count (⟨seed⟩, (none, none))

/-- `generate`: `list[index]` for each chosen index. -/
def generate {α : Type} [Inhabited α] (list : List α) (seed : UInt64) (count : Nat) : List α :=
  (generateIndices list.length seed count).map (list[·]!)

/-- `server/words/english.txt` holds 200 words (`word_list_is_lowercase_ascii_and_unique`). -/
def englishLen : Nat := 200

#guard generateIndices englishLen 0 5 = [176, 85, 5, 194, 22]

namespace SplitMix64

theorem scale_lt (x : UInt64) {bound : Nat} (h : 0 < bound) : scale x bound < bound := by
  unfold scale
  rw [Nat.div_lt_iff_lt_mul (by decide), Nat.mul_comm]
  exact Nat.mul_lt_mul_of_pos_left x.toNat_lt h

theorem below_lt (rng : SplitMix64) {bound : Nat} (h : 0 < bound) :
    (rng.below bound).1 < bound :=
  scale_lt _ h

end SplitMix64

/-- What the sort and `flatten` hand the fold: the excluded indices, ascending. -/
theorem skips_shape {bound : Nat} {excluded : Excluded} (h : ExcludedOk bound excluded) :
    (flatten (sortExcluded excluded) = [] ∧ ∀ i, ¬ IsExcluded excluded i) ∨
    (∃ a, flatten (sortExcluded excluded) = [a] ∧ a < bound ∧
      ∀ i, IsExcluded excluded i ↔ i = a) ∨
    (∃ a b, flatten (sortExcluded excluded) = [a, b] ∧ a < b ∧ b < bound ∧
      ∀ i, IsExcluded excluded i ↔ i = a ∨ i = b) := by
  rcases excluded with ⟨_ | a, _ | b⟩ <;> simp [ExcludedOk] at h
  · simp [flatten, sortExcluded, optionLe, IsExcluded]
  · exact .inr (.inl ⟨b, by simp [flatten, sortExcluded, optionLe], h,
      by simp [IsExcluded, eq_comm]⟩)
  · exact .inr (.inl ⟨a, by simp [flatten, sortExcluded, optionLe], h,
      by simp [IsExcluded, eq_comm]⟩)
  · refine .inr (.inr ?_)
    by_cases hab : a ≤ b
    · exact ⟨a, b, by simp [flatten, sortExcluded, optionLe, hab], by omega, h.2.2,
        by simp [IsExcluded]; grind⟩
    · exact ⟨b, a, by simp [flatten, sortExcluded, optionLe, hab], by omega, h.2.1,
        by simp [IsExcluded]; grind⟩

/-- `bound - count` in `below_excluding` never underflows once the contract holds. -/
theorem allowed_no_underflow {bound : Nat} {excluded : Excluded}
    (h : ExcludedOk bound excluded) : (flatten (sortExcluded excluded)).length ≤ bound := by
  rcases skips_shape h with ⟨hs, -⟩ | ⟨a, hs, ha, -⟩ | ⟨a, b, hs, hab, hb, -⟩ <;>
    simp [hs] <;> omega

/-- A draw below `allowed` lands below `bound` on an index that is not excluded. -/
theorem placeDraw_ok {bound : Nat} {excluded : Excluded} (h : ExcludedOk bound excluded)
    {draw : Nat} (hd : draw < allowed bound excluded) :
    placeDraw excluded draw < bound ∧ ¬ IsExcluded excluded (placeDraw excluded draw) := by
  unfold allowed at hd
  unfold placeDraw
  rcases skips_shape h with ⟨hs, hx⟩ | ⟨a, hs, ha, hx⟩ | ⟨a, b, hs, hab, hb, hx⟩ <;>
    simp only [hs, hx] at hd ⊢ <;> simp at hd ⊢ <;> grind

/-- Distinct draws land on distinct indices. -/
theorem placeDraw_injective {bound : Nat} {excluded : Excluded} (h : ExcludedOk bound excluded)
    {d₁ d₂ : Nat} (he : placeDraw excluded d₁ = placeDraw excluded d₂) : d₁ = d₂ := by
  unfold placeDraw at he
  rcases skips_shape h with ⟨hs, -⟩ | ⟨a, hs, -, -⟩ | ⟨a, b, hs, hab, -, -⟩ <;>
    simp only [hs] at he <;> simp at he <;> grind

/-- Every index below `bound` that is not excluded is the landing spot of some draw below
`allowed`. With `placeDraw_injective`, the fold is a bijection from `[0, allowed)` onto the
allowed indices, so a uniform draw gives a uniform allowed index. -/
theorem placeDraw_surjective {bound : Nat} {excluded : Excluded} (h : ExcludedOk bound excluded)
    {i : Nat} (hi : i < bound) (hx : ¬ IsExcluded excluded i) :
    ∃ draw, draw < allowed bound excluded ∧ placeDraw excluded draw = i := by
  unfold allowed placeDraw
  rcases skips_shape h with ⟨hs, -⟩ | ⟨a, hs, ha, hx'⟩ | ⟨a, b, hs, hab, hb, hx'⟩ <;>
    simp only [hs] <;> simp
  · exact hi
  · have : i ≠ a := fun e => hx ((hx' i).2 e)
    refine ⟨if i < a then i else i - 1, ?_, ?_⟩ <;> grind
  · have : i ≠ a ∧ i ≠ b :=
      ⟨fun e => hx ((hx' i).2 (.inl e)), fun e => hx ((hx' i).2 (.inr e))⟩
    refine ⟨if i < a then i else if i < b then i - 1 else i - 2, ?_, ?_⟩ <;> grind

theorem flatten_sortExcluded_length (excluded : Excluded) :
    (flatten (sortExcluded excluded)).length = (flatten excluded).length := by
  rcases excluded with ⟨_ | a, _ | b⟩ <;> simp [flatten, sortExcluded, optionLe] <;>
    split <;> simp

/-- `below_excluding` returns an index below `bound` that is none of the excluded ones, given
its contract and more indices than exclusions. -/
theorem SplitMix64.belowExcluding_ok (rng : SplitMix64) {bound : Nat} {excluded : Excluded}
    (h : ExcludedOk bound excluded) (hc : (flatten excluded).length < bound) :
    (rng.belowExcluding bound excluded).1 < bound ∧
      ¬ IsExcluded excluded (rng.belowExcluding bound excluded).1 := by
  apply placeDraw_ok h
  apply rng.below_lt
  unfold allowed
  rw [flatten_sortExcluded_length]
  omega

theorem excludedOk_shift {len : Nat} {p q : Option Nat} {i : Nat} (h : ExcludedOk len (p, q))
    (hi : i < len) (hx : ¬ IsExcluded (p, q) i) : ExcludedOk len (q, some i) := by
  rcases p with _ | a <;> rcases q with _ | b <;> simp_all [ExcludedOk, IsExcluded] <;> omega

theorem flatten_length_le (excluded : Excluded) : (flatten excluded).length ≤ 2 := by
  rcases excluded with ⟨_ | a, _ | b⟩ <;> simp [flatten]

theorem generateStep_ok {len : Nat} (hlen : 3 ≤ len) {s : SplitMix64 × Excluded}
    (h : ExcludedOk len s.2) :
    (generateStep len s).1 < len ∧ ¬ IsExcluded s.2 (generateStep len s).1 ∧
      ExcludedOk len (generateStep len s).2.2 := by
  have := s.1.belowExcluding_ok h (by have := flatten_length_le s.2; omega)
  rcases s with ⟨rng, p, q⟩
  exact ⟨this.1, this.2, excludedOk_shift h this.1 this.2⟩

theorem generateStep_recent (len : Nat) (s : SplitMix64 × Excluded) :
    (generateStep len s).2.2 = (s.2.2, some (generateStep len s).1) := rfl

theorem generateLoop_succ (len n : Nat) (s : SplitMix64 × Excluded) :
    generateLoop len (n + 1) s =
      (generateStep len s).1 :: generateLoop len n (generateStep len s).2 :=
  rfl

theorem generateLoop_length (len n : Nat) (s : SplitMix64 × Excluded) :
    (generateLoop len n s).length = n := by
  induction n generalizing s with
  | zero => rfl
  | succ n ih => simp [generateLoop_succ, ih]

theorem generateState_step (len k : Nat) (s : SplitMix64 × Excluded) :
    generateState len k (generateStep len s).2 = generateState len (k + 1) s := by
  induction k with
  | zero => simp only [generateState]
  | succ k ih => simp only [generateState, ih]

/-- The `k`th index is what `below_excluding` returns in the loop's `k`th iteration. -/
theorem generateLoop_getElem? (len n k : Nat) (s : SplitMix64 × Excluded) :
    (generateLoop len n s)[k]? =
      if k < n then some (generateStep len (generateState len k s)).1 else none := by
  induction n generalizing s k with
  | zero => simp [generateLoop]
  | succ n ih =>
    cases k with
    | zero => simp [generateLoop_succ, generateState]
    | succ k => simp [generateLoop_succ, ih, generateState_step]

/-- Every call to `below_excluding` in `generate` meets its contract: the `k`th iteration's
`recent` is distinct and in range, and the list is longer than its two entries. -/
theorem generate_contract {len : Nat} (hlen : 3 ≤ len) (seed : UInt64) (k : Nat) :
    ExcludedOk len (generateState len k (⟨seed⟩, (none, none))).2 ∧
      (flatten (generateState len k (⟨seed⟩, (none, none))).2).length < len := by
  refine ⟨?_, Nat.lt_of_le_of_lt (flatten_length_le _) hlen⟩
  induction k with
  | zero => trivial
  | succ k ih => simpa only [generateState] using (generateStep_ok hlen ih).2.2

theorem generateLoop_ok {len : Nat} (hlen : 3 ≤ len) (n : Nat) (s : SplitMix64 × Excluded)
    (h : ExcludedOk len s.2) :
    (∀ x ∈ generateLoop len n s, x < len) ∧
    (∀ k a b, (generateLoop len n s)[k]? = some a → (generateLoop len n s)[k + 1]? = some b →
      a ≠ b) ∧
    (∀ k a b, (generateLoop len n s)[k]? = some a → (generateLoop len n s)[k + 2]? = some b →
      a ≠ b) ∧
    (∀ a, (generateLoop len n s)[0]? = some a → ¬ IsExcluded s.2 a) ∧
    (∀ a, (generateLoop len n s)[1]? = some a → s.2.2 ≠ some a) := by
  induction n generalizing s with
  | zero => simp [generateLoop]
  | succ n ih =>
    obtain ⟨hi, hx, h'⟩ := generateStep_ok hlen h
    obtain ⟨r1, r2, r3, r4, r5⟩ := ih _ h'
    rw [generateStep_recent] at r4 r5
    rw [generateLoop_succ]
    generalize generateLoop len n (generateStep len s).2 = l at r1 r2 r3 r4 r5
    generalize (generateStep len s).1 = i at hi hx r4 r5
    simp only [IsExcluded] at r4 hx
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · simpa [hi] using r1
    · intro k a b ha hb
      cases k with
      | zero => simp at ha hb; subst ha; have := r4 b hb; simp at this; omega
      | succ k => exact r2 k a b ha hb
    · intro k a b ha hb
      cases k with
      | zero => simp at ha hb; subst ha; have := r5 b hb; simp at this; omega
      | succ k => exact r3 k a b ha hb
    · intro a ha; simp at ha; subst ha; exact hx
    · intro a ha; have := r4 a (by simpa using ha); grind

theorem generateIndices_length (len : Nat) (seed : UInt64) (count : Nat) :
    (generateIndices len seed count).length = count :=
  generateLoop_length _ _ _

theorem generateIndices_lt {len : Nat} (hlen : 3 ≤ len) (seed : UInt64) (count : Nat) :
    ∀ x ∈ generateIndices len seed count, x < len :=
  (generateLoop_ok hlen count (⟨seed⟩, (none, none)) trivial).1

/-- No index equals either of the two before it. -/
theorem generateIndices_never_repeats {len : Nat} (hlen : 3 ≤ len) (seed : UInt64)
    (count k : Nat) {a b : Nat} (ha : (generateIndices len seed count)[k]? = some a) :
    ((generateIndices len seed count)[k + 1]? = some b → a ≠ b) ∧
    ((generateIndices len seed count)[k + 2]? = some b → a ≠ b) :=
  have h := generateLoop_ok hlen count (⟨seed⟩, (none, none)) trivial
  ⟨h.2.1 k a b ha, h.2.2.1 k a b ha⟩

theorem generateLoop_take (len : Nat) {n m : Nat} (h : n ≤ m) (s : SplitMix64 × Excluded) :
    (generateLoop len m s).take n = generateLoop len n s := by
  induction n generalizing m s with
  | zero => simp [generateLoop]
  | succ n ih =>
    cases m with
    | zero => omega
    | succ m => simp [generateLoop_succ, ih (Nat.le_of_succ_le_succ h)]

/-- Asking for more words only appends: a shorter test's text is a prefix of a longer one's, so
changing a mode's word count keeps the text of tokens already issued as a prefix. -/
theorem generateIndices_prefix (len : Nat) (seed : UInt64) {n m : Nat} (h : n ≤ m) :
    generateIndices len seed n <+: generateIndices len seed m := by
  rw [generateIndices, generateIndices, ← generateLoop_take len h]
  exact List.take_prefix _ _

theorem generate_length {α : Type} [Inhabited α] (list : List α) (seed : UInt64) (count : Nat) :
    (generate list seed count).length = count := by
  simp [generate, generateIndices_length]

theorem generate_prefix {α : Type} [Inhabited α] (list : List α) (seed : UInt64) {n m : Nat}
    (h : n ≤ m) : generate list seed n <+: generate list seed m :=
  (generateIndices_prefix _ _ h).map _

/-- For a word list without duplicates, no word equals either of the two before it. -/
theorem generate_never_repeats {α : Type} [Inhabited α] {list : List α} (hn : list.Nodup)
    (hlen : 3 ≤ list.length) (seed : UInt64) (count k : Nat) {a b : α}
    (ha : (generate list seed count)[k]? = some a) :
    ((generate list seed count)[k + 1]? = some b → a ≠ b) ∧
    ((generate list seed count)[k + 2]? = some b → a ≠ b) := by
  simp only [generate, List.getElem?_map] at ha ⊢
  obtain ⟨i, hi, rfl⟩ := Option.map_eq_some_iff.mp ha
  have hil := generateIndices_lt hlen seed count i (List.mem_of_getElem? hi)
  have key : ∀ d, ∀ j, (generateIndices list.length seed count)[k + d]? = some j →
      i ≠ j → list[i]! ≠ list[j]! := by
    intro d j hj hij e
    have hjl := generateIndices_lt hlen seed count j (List.mem_of_getElem? hj)
    rw [getElem!_pos list i hil, getElem!_pos list j hjl] at e
    exact hij ((List.Nodup.getElem_inj hn).mp e)
  constructor <;> intro hb <;> obtain ⟨j, hj, rfl⟩ := Option.map_eq_some_iff.mp hb
  · exact key 1 j hj ((generateIndices_never_repeats hlen seed count k hi).1 hj)
  · exact key 2 j hj ((generateIndices_never_repeats hlen seed count k hi).2 hj)

/-! `below` maps the `2^64` draws onto `[0, bound)` with no rejection step, so it is uniform only
up to rounding. The theorems below bound that bias exactly. -/

theorem countP_lt_range (n b t : Nat) (hb : 0 < b) :
    (List.range n).countP (fun x => x * b < t) = min n ((t + b - 1) / b) := by
  induction n with
  | zero => simp
  | succ n ih =>
    rw [List.range_succ, List.countP_append, ih]
    have h : n < (t + b - 1) / b ↔ n * b < t := by
      rw [← Nat.succ_le_iff, Nat.le_div_iff_mul_le hb, Nat.succ_mul]; omega
    by_cases hn : n * b < t <;> simp [hn] <;> have := h.mpr <;> have := h.mp <;> grind

theorem countP_window (n b lo hi : Nat) (h : lo ≤ hi) :
    (List.range n).countP (fun x => lo ≤ x * b ∧ x * b < hi) =
      (List.range n).countP (fun x => x * b < hi) -
        (List.range n).countP (fun x => x * b < lo) := by
  have : ∀ n, (List.range n).countP (fun x => lo ≤ x * b ∧ x * b < hi) +
      (List.range n).countP (fun x => x * b < lo) =
        (List.range n).countP (fun x => x * b < hi) := by
    intro n
    induction n with
    | zero => simp
    | succ n ih =>
      simp only [List.range_succ, List.countP_append]
      by_cases h1 : n * b < lo <;> by_cases h2 : n * b < hi <;> simp [h1, h2] at ih ⊢ <;>
        (try split) <;> omega
  have := this n; omega

theorem div_eq_iff_window {x n i : Nat} (hn : 0 < n) :
    x / n = i ↔ i * n ≤ x ∧ x < (i + 1) * n := by
  constructor
  · rintro rfl
    exact ⟨Nat.div_mul_le_self x n, by rw [Nat.succ_mul]; exact Nat.lt_div_mul_add hn⟩
  · rintro ⟨h1, h2⟩
    exact Nat.div_eq_of_lt_le h1 h2

/-- Consecutive ceilings of multiples of `N / b` differ by `floor(N / b)` or `ceil(N / b)`. -/
theorem ceil_gap (N b i : Nat) (hb : 0 < b) :
    ((i + 1) * N + b - 1) / b - (i * N + b - 1) / b = N / b ∨
    ((i + 1) * N + b - 1) / b - (i * N + b - 1) / b = (N + b - 1) / b := by
  have hN := Nat.div_add_mod N b
  generalize N / b = q at hN ⊢
  have hr := Nat.mod_lt N hb
  generalize N % b = r at hN hr
  subst hN
  have e1 : (i + 1) * (b * q + r) + b - 1 = ((i + 1) * r + b - 1) + b * ((i + 1) * q) := by
    simp only [Nat.mul_add, Nat.add_mul, Nat.one_mul, Nat.mul_left_comm]; omega
  have e2 : i * (b * q + r) + b - 1 = (i * r + b - 1) + b * (i * q) := by
    simp only [Nat.mul_add, Nat.mul_left_comm]; omega
  have e3 : b * q + r + b - 1 = (r + b - 1) + b * q := by omega
  rw [e1, e2, e3, Nat.add_mul_div_left _ _ hb, Nat.add_mul_div_left _ _ hb,
    Nat.add_mul_div_left _ _ hb]
  have e4 : (i + 1) * r + b - 1 = (i * r + b - 1) + r := by
    simp only [Nat.add_mul, Nat.one_mul]; omega
  rw [e4]
  generalize i * r + b - 1 = u
  have lo : u / b ≤ (u + r) / b := Nat.div_le_div_right (by omega)
  have hi : (u + r) / b ≤ u / b + 1 := by
    rw [← Nat.add_div_right u hb]; exact Nat.div_le_div_right (by omega)
  have hq : (i + 1) * q = i * q + q := Nat.succ_mul i q
  by_cases hr0 : r = 0
  · subst hr0; left; rw [Nat.add_zero]; omega
  · have h1 : (r + b - 1) / b = 1 := Nat.div_eq_of_lt_le (by omega) (by omega)
    rw [h1]; omega

/-- How many `x < N` have `x * b / N = i`: `floor(N / b)` or `ceil(N / b)`. -/
theorem window_count {N b i : Nat} (hN : 0 < N) (hb : 0 < b) (hi : i < b) :
    (List.range N).countP (fun x => x * b / N = i) = N / b ∨
    (List.range N).countP (fun x => x * b / N = i) = (N + b - 1) / b := by
  have hc : (List.range N).countP (fun x => x * b / N = i) =
      (List.range N).countP (fun x => i * N ≤ x * b ∧ x * b < (i + 1) * N) :=
    List.countP_congr (fun x _ => by simp [div_eq_iff_window hN])
  have hle : ∀ j, j ≤ b → (j * N + b - 1) / b ≤ N := fun j hj => by
    have : j * N ≤ b * N := Nat.mul_le_mul_right N hj
    have : (j * N + b - 1) / b < N + 1 := by
      rw [Nat.div_lt_iff_lt_mul hb, Nat.succ_mul, Nat.mul_comm N b]; omega
    omega
  rw [hc, countP_window _ _ _ _ (Nat.mul_le_mul_right N (Nat.le_succ i)),
    countP_lt_range _ _ _ hb, countP_lt_range _ _ _ hb,
    Nat.min_eq_right (hle _ hi), Nat.min_eq_right (hle _ (Nat.le_of_lt hi))]
  exact ceil_gap N b i hb

/-- Each index in `[0, bound)` is the `below` result of either `floor(2^64 / bound)` or
`ceil(2^64 / bound)` of the `2^64` possible draws (`UInt64.ofNat x` for `x < 2^64` lists every
`u64` once). So `below` is uniform up to one draw in `2^64` per index. -/
theorem SplitMix64.scale_hits {bound i : Nat} (hb : 0 < bound) (hi : i < bound) :
    ((List.range (2 ^ 64)).countP (fun x => SplitMix64.scale (UInt64.ofNat x) bound = i) =
      2 ^ 64 / bound) ∨
    ((List.range (2 ^ 64)).countP (fun x => SplitMix64.scale (UInt64.ofNat x) bound = i) =
      (2 ^ 64 + bound - 1) / bound) := by
  have : (List.range (2 ^ 64)).countP (fun x => SplitMix64.scale (UInt64.ofNat x) bound = i) =
      (List.range (2 ^ 64)).countP (fun x => x * bound / 2 ^ 64 = i) :=
    List.countP_congr fun x hx => by
      simp [SplitMix64.scale, UInt64.toNat_ofNat_of_lt' (List.mem_range.mp hx)]
  rw [this]
  exact window_count (by decide) hb hi

end Platen
