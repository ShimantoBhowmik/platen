/-!
# Speed chart scales (`web/src/chart.rs`)

The top of the error axis and the spacing of the seconds ruler. Abstractions: `Nat` for the
`u32`/`usize` counts. `errors_max * wpm_lines + 1` cannot overflow: `errors_max` is the most
wrong keys in one second and `wpm_lines` is at most `MAX_WPM_LINES = 4`. `label_every` takes
its float `plot_width` only through `room = (plot_width / 26.0).max(1.0) as usize`, so the model
takes `room` as that natural.
-/

namespace Platen

/-- `u32::is_multiple_of`: zero is the only multiple of zero. -/
def isMultipleOf (x n : Nat) : Bool := if n = 0 then x == 0 else x % n == 0

/-- The predicate `errors_scale` searches with: no error count in `1..=errors_max` times
`wpm_lines` is a multiple of `top`. -/
def clearsGridlines (errorsMax wpmLines top : Nat) : Bool :=
  (List.range' 1 errorsMax).all fun errors => !isMultipleOf (errors * wpmLines) top

/-- `errors_scale`: the first `top` in `errors_max + 1..=errors_max * wpm_lines + 1` that clears
the gridlines, else `errors_max * wpm_lines + 1`. -/
def errorsScale (errorsMax wpmLines : Nat) : Nat :=
  ((List.range' (errorsMax + 1) (errorsMax * wpmLines + 1 + 1 - (errorsMax + 1))).find?
      (clearsGridlines errorsMax wpmLines)).getD (errorsMax * wpmLines + 1)

/-- The doc's "puts no error mark on a WPM gridline": no error count `e` in `1..=errors_max`
sits at the height of a gridline `line` in `0..=wpm_lines`, i.e. `e / top = line / wpm_lines`. -/
def offGridlines (errorsMax wpmLines top : Nat) : Prop :=
  ∀ e, 1 ≤ e → e ≤ errorsMax → ∀ line, line ≤ wpmLines → e * wpmLines ≠ line * top

/-- With a top above every error count, clearing the divisibility test is exactly staying off
the gridlines: a multiple `line * top` of `e * wpm_lines < top * wpm_lines` has `line < wpm_lines`. -/
theorem clearsGridlines_iff {errorsMax wpmLines top : Nat} (htop : errorsMax < top) :
    clearsGridlines errorsMax wpmLines top = true ↔ offGridlines errorsMax wpmLines top := by
  have htop0 : top ≠ 0 := by omega
  simp only [clearsGridlines, isMultipleOf, htop0, ite_false, List.all_eq_true, List.mem_range'_1,
    Bool.not_eq_eq_eq_not, Bool.not_true, beq_eq_false_iff_ne, ne_eq, offGridlines]
  constructor
  · intro h e he1 he2 line _ heq
    exact h e ⟨he1, by omega⟩ (by rw [heq, Nat.mul_mod_left])
  · intro h e ⟨he1, he2⟩ hmod
    obtain ⟨line, hline⟩ := Nat.dvd_of_mod_eq_zero hmod
    refine h e he1 (by omega) line ?_ (by rw [hline, Nat.mul_comm])
    -- `line * top = e * wpm_lines ≤ errors_max * wpm_lines < top * wpm_lines`.
    have h1 : top * line ≤ top * wpmLines := by
      rw [← hline]; exact Nat.mul_le_mul_right _ (by omega)
    exact Nat.le_of_mul_le_mul_left h1 (by omega)

/-- The fallback `errors_max * wpm_lines + 1` already clears the gridlines: every `e * wpm_lines`
is positive and below it. -/
theorem fallback_clears {errorsMax wpmLines : Nat} (hl : 1 ≤ wpmLines) :
    clearsGridlines errorsMax wpmLines (errorsMax * wpmLines + 1) = true := by
  simp only [clearsGridlines, isMultipleOf, List.all_eq_true, List.mem_range'_1,
    Nat.add_one_ne_zero, ite_false, Bool.not_eq_eq_eq_not, Bool.not_true, beq_eq_false_iff_ne]
  intro e ⟨he1, he2⟩
  have hlt : e * wpmLines < errorsMax * wpmLines + 1 :=
    Nat.lt_succ_of_le (Nat.mul_le_mul_right _ (by omega))
  have hpos : 0 < e * wpmLines := Nat.mul_pos (by omega) hl
  rw [Nat.mod_eq_of_lt hlt]; omega

/-- The search always finds a top, so `unwrap_or` is never reached. -/
theorem errorsScale_found {errorsMax wpmLines : Nat} (hl : 1 ≤ wpmLines) :
    ((List.range' (errorsMax + 1) (errorsMax * wpmLines + 1 + 1 - (errorsMax + 1))).find?
      (clearsGridlines errorsMax wpmLines)).isSome := by
  rw [Option.isSome_iff_ne_none, ne_eq, List.find?_eq_none]
  intro h
  have hmem : errorsMax * wpmLines + 1 ∈
      List.range' (errorsMax + 1) (errorsMax * wpmLines + 1 + 1 - (errorsMax + 1)) := by
    have : errorsMax ≤ errorsMax * wpmLines := Nat.le_mul_of_pos_right _ hl
    rw [List.mem_range'_1]; omega
  exact h _ hmem (fallback_clears hl)

/-- The search result is the first top in its range that clears the gridlines. -/
theorem errorsScale_spec {errorsMax wpmLines : Nat} (hl : 1 ≤ wpmLines) :
    errorsMax < errorsScale errorsMax wpmLines ∧
      clearsGridlines errorsMax wpmLines (errorsScale errorsMax wpmLines) = true ∧
      ∀ t, errorsMax < t → t < errorsScale errorsMax wpmLines →
        clearsGridlines errorsMax wpmLines t = false := by
  obtain ⟨r, hr⟩ := Option.isSome_iff_exists.mp (errorsScale_found (errorsMax := errorsMax) hl)
  obtain ⟨h1, h2, h3⟩ := List.find?_range'_eq_some.mp hr
  rw [List.mem_range'_1] at h2
  simp only [errorsScale, hr, Option.getD_some]
  exact ⟨by omega, h1, fun t ht1 ht2 => by simpa using h3 t (by omega) ht2⟩

/-- The error axis tops out above every error count. -/
theorem errorsScale_gt {errorsMax wpmLines : Nat} (hl : 1 ≤ wpmLines) :
    errorsMax < errorsScale errorsMax wpmLines :=
  (errorsScale_spec hl).1

/-- No error mark lands on a WPM gridline. -/
theorem errorsScale_offGridlines {errorsMax wpmLines : Nat} (hl : 1 ≤ wpmLines) :
    offGridlines errorsMax wpmLines (errorsScale errorsMax wpmLines) :=
  (clearsGridlines_iff (errorsScale_gt hl)).mp (errorsScale_spec hl).2.1

/-- The result is the smallest count above `errors_max` that keeps marks off the gridlines. -/
theorem errorsScale_least {errorsMax wpmLines top : Nat} (hl : 1 ≤ wpmLines)
    (htop : errorsMax < top) (hoff : offGridlines errorsMax wpmLines top) :
    errorsScale errorsMax wpmLines ≤ top := by
  apply Nat.le_of_not_lt
  intro hlt
  have := (errorsScale_spec hl).2.2 top htop hlt
  rw [(clearsGridlines_iff htop).mpr hoff] at this
  exact Bool.noConfusion this

#guard errorsScale 2 4 = 3
#guard errorsScale 1 4 = 3
#guard errorsScale 3 3 = 4
#guard errorsScale 1 1 = 2
#guard [(1, 2), (4, 4), (7, 3), (12, 4)].all fun (errorsMax, lines) =>
  let top := errorsScale errorsMax lines
  decide (errorsMax < top) && clearsGridlines errorsMax lines top

/-- `usize::div_ceil` for a positive divisor. -/
def divCeil (a b : Nat) : Nat := (a + b - 1) / b

/-- The label spacings `label_every` may pick, smallest first. -/
def labelSpacings : List Nat := [1, 2, 5, 10, 15, 30, 60]

/-- `label_every`, with `room` the number of labels that fit. -/
def labelEvery (seconds room : Nat) : Nat :=
  (labelSpacings.find? fun every => decide (divCeil seconds every ≤ room)).getD 60

/-- The spacing is one of the listed steps. -/
theorem labelEvery_mem (seconds room : Nat) : labelEvery seconds room ∈ labelSpacings := by
  unfold labelEvery
  cases h : labelSpacings.find? fun every => decide (divCeil seconds every ≤ room) with
  | none => decide
  | some every => exact List.mem_of_find?_eq_some h

/-- A test of at most `60 * room` seconds gets labels that fit in the room. -/
theorem labelEvery_fits {seconds room : Nat} (h : seconds ≤ 60 * room) :
    divCeil seconds (labelEvery seconds room) ≤ room := by
  unfold labelEvery
  cases hf : labelSpacings.find? fun every => decide (divCeil seconds every ≤ room) with
  | none =>
    have := List.find?_eq_none.mp hf 60 (by decide)
    simp only [divCeil, Option.getD_none] at this ⊢
    omega
  | some every => simpa using List.find?_some hf

-- `room` for a 900 px plot: `900 / 26` is 34.6, cast down to 34.
#guard labelEvery 15 (900 / 26) = 1
#guard labelEvery 120 (900 / 26) = 5
#guard labelEvery 120 (300 / 26) = 15

end Platen
