# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Frontend: Dioxus web (Rust compiled to WASM), chosen by the user. It follows the repo's
CLAUDE.md rule that deep computation stays in Rust, and it can share domain types such as
test modes with the backend.

Backend (exists): Rust, axum and Postgres in `src/`. Documented in `README.md`.

## Users

People learning to touch type. They come to build skill, not to show off speed. Accuracy and
their own mistakes matter more to them than rank. They come back across many short sessions
to practise.

## Product Purpose

type-cast is a typing test that helps learners get better. A test should start right away.
Afterward the learner should understand what went wrong and what to practise next. Success
means a learner returns and their accuracy and speed rise over time.

## Positioning

Speed and no friction. No account and no setup: the page loads instantly and typing starts
within a second. Every score is also verified on the server from the keystroke log, so
leaderboard numbers are real (see `README.md`). The user did not choose this as the headline
difference.

## Operating Context

- The learner sits at a physical keyboard. The test runs in a desktop browser.
- Sessions are short: one test lasts 15 to 120 seconds or 10 to 100 words, often several in a
  row.
- The client logs every keystroke with a timestamp and sends the log to the server, which
  scores it. The client never reports its own score.

## Capabilities and Constraints

- Test modes: time 15/30/60/120 s and words 10/25/50/100. The text comes from a fixed list of
  the 200 most common English words.
- Scores: WPM, raw WPM, accuracy and consistency, computed on the server.
- Leaderboard: optional display name, best result per name per mode. Names are not protected
  because there are no accounts.
- Share links: every result has a URL (`GET /api/results/{id}`).
- Per-test feedback: each result includes a mistake breakdown, computed on the server from
  the keystroke log. It ranks the learner's weakest keys and lists the words left wrong. The
  breakdown is stored, so share links show it too.
- No result history. Neither the browser nor the server keeps a learner's past results.
  There are no accounts.
- Theme: adaptive light and dark by default, with a toggle (repo CLAUDE.md rule).
- Speed cap: results above 350 raw WPM are rejected.

## Brand Commitments

- The name is type-cast, and it is final.
- No logo, voice, or tagline exists yet.

## Evidence on Hand

- A working backend and API, documented in `README.md`.
- No testimonials, user counts, benchmarks, or press. Do not invent any.

## Product Principles

1. Typing starts at once. Any step between arriving and typing must justify itself.
2. Feedback teaches. Each result should show the learner what to practise next, not just a
   number.
3. Numbers are honest. Scores come only from server-verified keystrokes.
4. The learner's attention stays on the text while they type.

## Accessibility & Inclusion

- WCAG 2.2 AA: contrast, visible focus, and screen-reader labels on results and controls.
