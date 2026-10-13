/-!
# Weakest-key order (`server/src/scoring.rs`)

`Replay::breakdown` sorts its key stats with `sort_unstable_by(weaker_first)`. Since Rust 1.81
the std sorts may panic when the comparator is not a total order, so this module proves that
`weaker_first` is one.

Abstractions: `Nat` for the `u32` counts and the `u64` products, and the key as its ASCII byte
(`breakdown` builds every key from a byte below 128, and `char` order on ASCII is byte order).
Neither the `u32` sum `hits + misses` nor the `u64` products can overflow: a submission has at
most `MAX_KEYSTROKES = 10_000` keys, so every count is at most 10,000 and every product at most
10^8 (`product_le`). Even unbounded `u32` operands give a product below 2^64.
-/

namespace Platen

/-- `KeyStat`. -/
structure KeyStat where
  key : Nat
  hits : Nat
  misses : Nat
  deriving DecidableEq, Repr

/-- `hits + misses`, the `attempts` closure of `weaker_first` and `KeyTally::attempts`. -/
def KeyStat.attempts (s : KeyStat) : Nat := s.hits + s.misses

/-- `weaker_first`: higher miss rate first (by cross-multiplying), then more misses, then the
key. -/
def weakerFirst (a b : KeyStat) : Ordering :=
  (compare (b.misses * a.attempts) (a.misses * b.attempts)).then
    ((compare b.misses a.misses).then (compare a.key b.key))

/-- `a` sorts before `b` exactly when one of the three stages decides it. -/
theorem weakerFirst_lt_iff {a b : KeyStat} :
    weakerFirst a b = .lt ↔
      b.misses * a.attempts < a.misses * b.attempts ∨
      (b.misses * a.attempts = a.misses * b.attempts ∧ b.misses < a.misses) ∨
      (b.misses * a.attempts = a.misses * b.attempts ∧ b.misses = a.misses ∧ a.key < b.key) := by
  simp only [weakerFirst, Ordering.then_eq_lt, Nat.compare_eq_lt, Nat.compare_eq_eq]
  omega

theorem weakerFirst_eq_iff {a b : KeyStat} :
    weakerFirst a b = .eq ↔
      b.misses * a.attempts = a.misses * b.attempts ∧ b.misses = a.misses ∧ a.key = b.key := by
  simp only [weakerFirst, Ordering.then_eq_eq, Nat.compare_eq_eq]

theorem weakerFirst_self (a : KeyStat) : weakerFirst a a = .eq :=
  weakerFirst_eq_iff.mpr ⟨rfl, rfl, rfl⟩

/-- Swapping the arguments reverses the order. -/
theorem weakerFirst_swap (a b : KeyStat) : weakerFirst a b = (weakerFirst b a).swap := by
  simp only [weakerFirst, Ordering.swap_then, Nat.compare_swap]

/-- Two stats compare equal only when they are for the same key. -/
theorem weakerFirst_eq_key {a b : KeyStat} (h : weakerFirst a b = .eq) : a.key = b.key :=
  (weakerFirst_eq_iff.mp h).2.2

/-- Among stats with distinct keys, as `breakdown` gives them, `eq` means the same stat. -/
theorem weakerFirst_eq_of_distinct {stats : List KeyStat}
    (hd : stats.Pairwise fun a b => a.key ≠ b.key) {a b : KeyStat} (ha : a ∈ stats)
    (hb : b ∈ stats) (h : weakerFirst a b = .eq) : a = b := by
  have hk := weakerFirst_eq_key h
  induction stats with
  | nil => simp at ha
  | cons s rest ih =>
    rw [List.pairwise_cons] at hd
    rcases List.mem_cons.mp ha with ha' | ha' <;> rcases List.mem_cons.mp hb with hb' | hb'
    · rw [ha', hb']
    · rw [ha'] at hk; exact absurd hk (hd.1 b hb')
    · rw [hb'] at hk; exact absurd hk.symm (hd.1 a ha')
    · exact ih hd.2 ha' hb'

/-- Chaining two cross-multiplied rate comparisons through `b`. Each case multiplies the first
by `C`, the second by `A`, and cancels `B`. -/
theorem rate_trans {A B C ma mb mc : Nat} (hA : 0 < A) (hB : 0 < B) (hC : 0 < C) :
    (mb * A ≤ ma * B → mc * B ≤ mb * C → mc * A ≤ ma * C) ∧
    (mb * A < ma * B → mc * B ≤ mb * C → mc * A < ma * C) ∧
    (mb * A ≤ ma * B → mc * B < mb * C → mc * A < ma * C) := by
  have chain : mc * B * A ≤ mb * C * A → mb * A * C ≤ ma * B * C →
      mc * A * B ≤ ma * C * B := by
    intro h1 h2
    have e1 : mc * A * B = mc * B * A := Nat.mul_right_comm _ _ _
    have e2 : mb * C * A = mb * A * C := Nat.mul_right_comm _ _ _
    have e3 : ma * B * C = ma * C * B := Nat.mul_right_comm _ _ _
    omega
  have chainLt : mc * B * A < mb * C * A ∨ mb * A * C < ma * B * C →
      mc * B * A ≤ mb * C * A → mb * A * C ≤ ma * B * C → mc * A * B < ma * C * B := by
    intro h h1 h2
    have e1 : mc * A * B = mc * B * A := Nat.mul_right_comm _ _ _
    have e2 : mb * C * A = mb * A * C := Nat.mul_right_comm _ _ _
    have e3 : ma * B * C = ma * C * B := Nat.mul_right_comm _ _ _
    omega
  refine ⟨fun h1 h2 => ?_, fun h1 h2 => ?_, fun h1 h2 => ?_⟩
  · exact Nat.le_of_mul_le_mul_right
      (chain (Nat.mul_le_mul_right _ h2) (Nat.mul_le_mul_right _ h1)) hB
  · exact Nat.lt_of_mul_lt_mul_right
      (chainLt (Or.inr (Nat.mul_lt_mul_of_pos_right h1 hC)) (Nat.mul_le_mul_right _ h2)
        (Nat.mul_le_mul_right _ (Nat.le_of_lt h1)))
  · exact Nat.lt_of_mul_lt_mul_right
      (chainLt (Or.inl (Nat.mul_lt_mul_of_pos_right h2 hA)) (Nat.mul_le_mul_right _ (Nat.le_of_lt h2))
        (Nat.mul_le_mul_right _ h1))

/-- `lt` is transitive on stats that each have an attempt, as `breakdown` keeps them. -/
theorem weakerFirst_trans_of_pos {a b c : KeyStat} (ha : 0 < a.attempts) (hb : 0 < b.attempts)
    (hc : 0 < c.attempts) (hab : weakerFirst a b = .lt) (hbc : weakerFirst b c = .lt) :
    weakerFirst a c = .lt := by
  rw [weakerFirst_lt_iff] at hab hbc ⊢
  obtain ⟨t1, t2, t3⟩ := rate_trans (ma := a.misses) (mb := b.misses) (mc := c.misses) ha hb hc
  obtain ⟨u1, -, -⟩ := rate_trans (ma := c.misses) (mb := b.misses) (mc := a.misses) hc hb ha
  rcases hab with hab | ⟨hab, hab'⟩ | ⟨hab, hab', hab''⟩ <;>
    rcases hbc with hbc | ⟨hbc, hbc'⟩ | ⟨hbc, hbc', hbc''⟩
  all_goals first
    | exact Or.inl (t2 (by omega) (by omega))
    | exact Or.inl (t3 (by omega) (by omega))
    | (have := t1 (by omega) (by omega); have := u1 (by omega) (by omega); omega)

/-- A stat with no attempts orders like one with a single hit: miss rate 0 and no misses. -/
def KeyStat.normalize (s : KeyStat) : KeyStat :=
  if s.attempts = 0 then { s with hits := 1 } else s

theorem KeyStat.normalize_attempts_pos (s : KeyStat) : 0 < s.normalize.attempts := by
  unfold normalize attempts; split <;> simp_all <;> omega

/-- Normalizing zero-attempt stats does not change any comparison. Such a stat ties every
other stat on miss rate (both cross products are 0), and has the fewest misses, so the misses
stage places it exactly where rate 0 would. -/
theorem weakerFirst_normalize (a b : KeyStat) :
    weakerFirst a b = weakerFirst a.normalize b.normalize := by
  have cmpZero : ∀ m : Nat, compare m 0 = if m = 0 then .eq else .gt := fun m => by
    split <;> simp_all [Nat.compare_eq_gt] <;> omega
  have zeroCmp : ∀ m : Nat, compare 0 m = if m = 0 then .eq else .lt := fun m => by
    split <;> simp_all [Nat.compare_eq_lt] <;> omega
  unfold KeyStat.normalize weakerFirst KeyStat.attempts
  by_cases ha : a.hits + a.misses = 0 <;> by_cases hb : b.hits + b.misses = 0 <;>
    simp only [ha, hb, ite_true, ite_false] <;>
    (try have : a.misses = 0 := by omega) <;> (try have : b.misses = 0 := by omega) <;>
    simp_all <;> split <;> simp_all

/-- `lt` is transitive on all stats, so `breakdown`'s positive-attempts filter is not what makes
the order total. -/
theorem weakerFirst_trans {a b c : KeyStat} (hab : weakerFirst a b = .lt)
    (hbc : weakerFirst b c = .lt) : weakerFirst a c = .lt := by
  rw [weakerFirst_normalize] at hab hbc ⊢
  exact weakerFirst_trans_of_pos a.normalize_attempts_pos b.normalize_attempts_pos
    c.normalize_attempts_pos hab hbc

/-- The `u64` products stay far below 2^64 under `MAX_KEYSTROKES`. -/
theorem product_le {a b : KeyStat} (ha : a.attempts ≤ 10000) (hb : b.attempts ≤ 10000) :
    b.misses * a.attempts ≤ 100000000 := by
  have : b.misses ≤ 10000 := by unfold KeyStat.attempts at hb; omega
  exact Nat.le_trans (Nat.mul_le_mul this ha) (by decide)

end Platen
