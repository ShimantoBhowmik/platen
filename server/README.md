# platen server

The backend: an axum HTTP server on Postgres. It issues tests, scores every submission itself
from the keystroke log, stores results and serves leaderboards. When `STATIC_DIR` is set, it
also serves the built frontend at `/`.

| Path | What it holds |
|---|---|
| `src/` | The server. `main.rs` wires the router; `scoring.rs` replays keystrokes. |
| `migrations/` | SQL migrations, embedded in the binary and run on startup. |
| `words/` | The word list tests draw from, with its license and attribution. |

## Run it

Start Postgres and create `.env` as the main README describes. Then, from this directory:

```bash
cargo run --release
```

## Configuration

The server reads these environment variables, or a `.env` file in the working directory or
a parent directory. `.env.example` lists them with defaults.

| Variable | Required | Use |
|---|---|---|
| `DATABASE_URL` | yes | Postgres connection string |
| `TOKEN_SECRET` | yes | Signs test tokens; at least 32 bytes |
| `BIND_ADDR` | no | Listen address; default `0.0.0.0:3000` |
| `STATIC_DIR` | no | Built frontend to serve at `/` |
| `MAX_DB_CONNECTIONS` | no | Postgres pool size; default 16 |
| `RUST_LOG` | no | Log filter |

## API

All bodies are JSON. Errors have the shape
`{ "error": "<code>", "message": "<text>", "reason"?: "<anti-cheat rule>" }`.

Modes are `{ "mode": "time", "amount": 15 | 30 | 60 | 120 }` or
`{ "mode": "words", "amount": 10 | 25 | 50 | 100 }`.

### `POST /api/tests`

Starts a test. Send a mode. The response holds an opaque `token` and the `words` to type. A
time test gets enough words for the 350 WPM keystroke cap.

The words are drawn from the 1,000 most common everyday English words in
`words/english.txt`. That list is derived from wordfreq and licensed CC BY-SA 4.0; see
`words/README.md` for the source, the filters and the attribution.

### `POST /api/results`

Submits a finished test. Returns `201` with the verified result.

```json
{
  "token": "<from /api/tests>",
  "keys": "the be\bo",
  "times": [0, 120, 260, 330, 470, 610, 800, 950],
  "name": "ada"
}
```

* `keys` is every key pressed, in order. `\b` is backspace.
* `times` has one entry per key: milliseconds since the first key.
* `name` is optional. A named result enters the leaderboard if it beats that name's best.

The client starts the clock on the first key. A time test ends at its duration; keys up to
100 ms late are ignored. A word test ends when the last word is typed correctly or a space
follows it. Backspace can return to the previous word only if that word was typed wrong.

| Status | `error`             | Meaning                                    |
|--------|---------------------|--------------------------------------------|
| 400    | `invalid_token`     | Token forged, altered, or malformed        |
| 400    | `invalid_name`      | Name empty, over 24 chars, or bad chars    |
| 409    | `already_submitted` | Each token can be scored once              |
| 422    | `result_rejected`   | Broke an anti-cheat rule, see `reason`     |

### `GET /api/results/{id}`

Returns a stored result, for share links.

### `GET /api/leaderboard?mode=time&amount=60&limit=50`

Returns the best result per name, fastest first. `limit` is 1 to 100 and defaults to 50.

### `GET /health`

Returns `200` while the process is up.

## Scoring

* WPM counts the characters of correctly typed words plus their spaces, divided by 5, per
  minute. A time test also credits the correct prefix of the last word.
* Raw WPM counts every character left in the final text, right or wrong, including the
  spaces between words. Characters deleted with backspace do not count.
* Accuracy is the percentage of character keystrokes that matched the text when typed.
  Corrected typos still count as errors.
* Consistency measures how steady the speed was. The server counts the characters typed in
  each second of the test (backspace excluded) and turns each count into a raw WPM. With
  `cv` as the coefficient of variation of those values (standard deviation divided by mean),
  consistency is `100 * (1 - tanh(cv + cv^3/3 + cv^5/5))`. For small `cv` this is about
  `100 * (1 - cv)`, and it falls smoothly toward 0. A word test also scores its last partial
  second if it lasts at least 0.5 s.
* The speed cap is 350 WPM of keystrokes: every character key, including ones later
  deleted, divided by 5 per minute. A faster submission is rejected.

## Mistake breakdown

Every result also has a `breakdown` that shows the learner what to practise:

```json
"breakdown": {
  "keys": [
    { "key": "r", "hits": 1, "misses": 1 },
    { "key": " ", "hits": 8, "misses": 1 }
  ],
  "missed_words": [{ "expected": "after", "typed": "aftez" }]
}
```

* `keys` lists every key the text asked for, weakest first. The order is miss rate, then
  miss count. Each keystroke counts toward the key the text expected at that position.
  Corrected typos count as misses.
* A space typed too early counts as a miss on the skipped letter. A space that commits a
  full-length wrong word counts as a missed space.
* Letters typed past the end of a word have no expected key. They count only in
  `incorrect_keystrokes`.
* `missed_words` lists the words left wrong when committed, in typing order. The unfinished
  last word of a time test is not listed.
* The breakdown is stored with the result, so share links show it too.

## Timeline

Every result also has a `timeline` for a speed chart, with one point per scored second:

```json
"timeline": [
  { "wpm": 84.0, "raw_wpm": 96.0, "errors": 0 },
  { "wpm": 78.0, "raw_wpm": 72.0, "errors": 1 }
]
```

* `wpm` is the WPM from the start of the test to the end of that second, counted like the
  result's WPM. The last point equals the result's WPM when the test ends on a scored second.
* `raw_wpm` is the raw WPM of the character keys typed in that second alone, rounded. These
  are the values that consistency measures.
* `errors` counts the character keystrokes in that second that did not match the text.
* The seconds follow the consistency rules: a word test also gets a point for its last
  partial second if it lasts at least 0.5 s.

## Design notes

* **Stateless tests.** The token is `uuid | issued_at | mode`, signed with HMAC-SHA256. The
  words are a pure function of the uuid, so the server stores nothing until a result arrives.
  Any instance can verify any token.
* **Single-use tokens.** A result's primary key is the test uuid.
* **Fast leaderboards.** A `leaderboard` table keeps each name's best per mode. One SQL
  statement inserts the result and updates that row. A covering index makes a leaderboard
  read an index-only scan.

