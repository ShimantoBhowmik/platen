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
- `Platen/Words.lean` models the word generator in `server/src/words.rs`, bit for bit: a
  `#guard` reproduces the sequence the Rust test pins. It proves that `below_excluding` maps
  its draws one to one onto the allowed indices, that a word never repeats either of the two
  before it, that each index is hit by `floor(2^64 / bound)` or `ceil(2^64 / bound)` draws,
  and that a shorter text is a prefix of a longer one from the same seed.
- `Platen/Replay.lean` models the keystroke replay in `server/src/scoring.rs` with the
  Rust's own string bookkeeping. `run_ok` proves that every accepted replay keeps the typed
  text well formed, so every slice and index in the Rust is in bounds. It also proves that
  the per-second marks ascend and that their counts add up to exactly the keystrokes the
  score counts. A correct word is never reopened (`step_lock`), and the WPM credit equals
  the committed correct words plus their spaces. `space_hit_iff` proves that a space counts
  as a hit exactly when it falls where the text has one: not on the last word, and after as
  many letters as the word has, right or wrong. `tooFast_sound` proves that a time test
  reaching its end is genuinely above the speed cap.
- `Platen/Session.lean` models the client's copy of those rules in `web/src/session.rs`.
  `word_test_refines` proves that, for any keys the learner presses, the server replays the
  client's key log to exactly the words the client shows. `time_test_refines` and
  `time_test_tooFast` prove the same for a time test, up to the point where the text runs
  out. There the server answers `TooFast`, and only above the speed cap.
- `Platen/Timeline.lean` models `timeline` in `server/src/scoring.rs`. It proves the chart has
  one point per scored second, that each point gets the keys and errors of its own second, and
  `timeline_last`: the last point counts the test's final characters through its full
  duration, so it equals the test's WPM.
- `Platen/KeyOrder.lean` models `weaker_first`, the comparator that sorts the weakest keys. It
  proves the comparator is a total order, which Rust's sort requires (since Rust 1.81 a sort
  may panic otherwise), and that its products cannot overflow.
- `Platen/Chart.lean` models `errors_scale` and `label_every` in `web/src/chart.rs`. It proves
  that no error mark lands on a WPM gridline, that the chosen scale is the smallest that
  achieves this, and that the ruler labels fit.
- `Platen/SheetLayout.lean` models the line wrapping in `Session::new` and `line_of` in
  `web/src/session.rs`. It proves every line fits the sheet unless it holds one long word,
  that wrapping is greedy, and that `line_of` finds the line holding each word.

## What the models leave out

The models are written by hand, so a proof covers the Rust only as far as the model matches
it. Each model keeps the Rust's branch structure, and its `#guard` checks rerun cases from the
Rust unit tests to catch a mismatch. When you change one of the Rust functions above, change
its model in the same branch.

Unsigned Rust integers are modelled as natural numbers. None of the modelled arithmetic comes
near overflow. Floating-point values (WPM, consistency, the rhythm check) are not modelled.
The integers they are computed from are.
