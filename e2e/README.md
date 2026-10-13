# e2e

An end-to-end check of the whole platform, run before a branch goes to CI. One command
starts a throwaway Postgres, builds and starts the API server, builds the frontend bundle it
serves, and drives a real browser with [Playwright](https://playwright.dev) at three
screen sizes. Afterwards it tears everything down.

## Run it

Requirements:

- Node 20 or later;
- the Rust toolchain;
- the Dioxus CLI (see `web/README.md`);
- Postgres binaries (`initdb`, `pg_ctl`, `createdb`).

```bash
cd e2e && npm ci
```

```bash
node run.mjs
```

The run ends with `== PASSED` (exit code 0) or `== FAILED` (exit code 1). Setup errors exit with 2.

| Option | Use |
|---|---|
| `--skip-build` | Reuse the last builds; for reruns when no Rust code changed |
| `--web debug` | Use the faster debug frontend build instead of the release bundle |
| `--grep <text>` | Run only checks whose title matches |
| `--project <name>` | Run one project: `api`, `desktop`, `laptop` or `phone` |
| `--headed` | Show the browser |
| `--keep` | Leave the stack running after the checks, until Ctrl-C |

`E2E_DATABASE_URL` points the run at an existing, disposable database instead of a throwaway
cluster. `PG_BIN` names the Postgres binary directory when it is not on `PATH`.

## What it checks

**API** (`tests/api.spec.mjs`, against the live server and database):
- health;
- every test mode;
- an unknown mode;
- a typed word test with deliberate mistakes. The check covers its stats, breakdown and per-second timeline, then fetching it back, the leaderboard entry and the single-use token;
- the anti-cheat rejections for early submission and machine-even rhythm;
- a tampered token and a bad name;
- an unknown result.

**Browser** (`tests/ui.spec.mjs`, at 1440x900, 1280x720 and a 390x844 phone):
- **A word test typed on the real keyboard with human timing.**
  - The countdown and live state while typing, and a first line of up to eleven words.
  - The result plate matches the server's verified numbers: WPM, accuracy, keys, one chart point per second, error marks, the word left wrong and the weakest keys.
  - Hovering or tapping a second shows its numbers.
  - Escape gives a fresh sheet.
  - The page never scrolls sideways, and on desktop sizes it fits the window.
- A 15 s time test counts down and is scored at its limit.
- Theme and sound choices survive a reload.
- Typing plays the recorded typewriter clips, with a different clack for each key in a row, the space bar plays its own soft thock and tick, and backspace plays its own lighter tick.
- Each test kind keeps the length last chosen for it.
- On a phone, a bar button does not move the page.
- Error states: a refused result and an unreachable server.
- Every page fails its check on any console error or uncaught exception.

## Output

`results/` is replaced on each run:

- `screens/`: full-page screenshots of each state at each size, taken after animations settle. Look at them; the checks cannot judge appearance.
- `server.log`: the API server's log.
- `report.json`: the Playwright results.
- `artifacts/`: a trace and screenshot for each failed check. Open one with `npx playwright show-trace <path>/trace.zip`.

## Measuring frontend performance

`perf.mjs` measures the frontend's main-thread cost in Chrome against a running stack. It
reports busy time per second while a time test runs with no keys, and busy time per key while
typing at a steady pace, split into script, style and layout. Start the stack with `--keep`,
then pass the URL it prints:

```bash
node run.mjs --keep
```

```bash
node perf.mjs --url http://127.0.0.1:<port> --runs 5
```

`--throttle 4` slows the CPU four times, close to a mid-range phone. Compare numbers only
from the same machine and the same throttle.

## Adding checks

Add a check to `tests/` when you add or change something a learner or API client can see.

- Type with `typeLikeHuman`. The server rejects a rhythm that is too even.
- Submit through the API with `humanGaps`.
- Use the `test` and `expect` exported by `tests/support.mjs`, so console errors fail the check.
- A failing check is a bug to fix, in the product or in the check. The suite never retries.
