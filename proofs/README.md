# proofs

Machine-checked models of the logic that decides a score, written in
[Lean 4](https://lean-lang.org). Each module transcribes one part of the Rust, function by
function, and proves the properties its doc comments promise. A promise that Lean cannot prove
is a bug: the model then gets a concrete counterexample, the Rust gets a fix and a regression
test, and the model is updated to the fixed code and proven.

## Check the proofs

Install [elan](https://github.com/leanprover/elan), the Lean toolchain manager. It reads the
pinned version from `lean-toolchain`.

```bash
cd proofs && lake build
```

A successful build means every theorem is proven. The build treats warnings as errors, so a
`sorry` (an unproven step) fails it. CI runs the same build.

## What is proven

- `Platen/Mode.lean` models `server/src/mode.rs`. It proves that mode codes and wire specs
  round-trip, and `time_text_outlasts_cap`. That theorem says a typist under the 350 WPM cap
  never reaches the end of a time test's text, which is why the replay may answer `TooFast`
  when one does. `time_wordCount_minimal` proves the text is no longer than that requires.
- `Platen/Marks.lean` holds the per-second record that the scoring replay keeps for the speed
  chart.

## What the models leave out

The models are written by hand, so a proof covers the Rust only as far as the model matches
it. Each model keeps the Rust's branch structure, and its `#guard` checks rerun cases from the
Rust unit tests to catch a mismatch. When you change one of the Rust functions above, change
its model in the same branch.

Unsigned Rust integers are modelled as natural numbers. None of the modelled arithmetic comes
near overflow. Floating-point values (WPM, consistency, the rhythm check) are not modelled.
The integers they are computed from are.
