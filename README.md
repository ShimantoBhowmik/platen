# type-cast

Backend for a typing-test website, written in Rust (axum + Postgres).

The server issues tests and scores them itself. A client never reports its own WPM: it sends
every keystroke with a timestamp, and the server replays them against the text it issued. This
blocks the easy cheats (posting a fake score, replaying a token, pasting text, scripted typing
at a fixed rhythm).

## Run it

```bash
docker compose up -d
```

```bash
cp .env.example .env
```

Set `TOKEN_SECRET` in `.env` to at least 32 random bytes, for example the output of
`openssl rand -base64 48`. Then start the server. Migrations run on startup.

```bash
cargo run --release
```

## API

All bodies are JSON. Errors have the shape
`{ "error": "<code>", "message": "<text>", "reason"?: "<anti-cheat rule>" }`.

Modes are `{ "mode": "time", "amount": 15 | 30 | 60 | 120 }` or
`{ "mode": "words", "amount": 10 | 25 | 50 | 100 }`.

### `POST /api/tests`

Starts a test. Send a mode. The response holds an opaque `token` and the `words` to type. A
time test gets enough words for the 350 WPM speed cap.

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
* Raw WPM counts every character keystroke, right or wrong.
* Accuracy is the percentage of character keystrokes that matched the text when typed.
* Consistency is `100 * (1 - cv)` of the intervals between keys. `cv` is the coefficient of
  variation: standard deviation divided by mean.

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

## Design notes

* **Stateless tests.** The token is `uuid | issued_at | mode`, signed with HMAC-SHA256. The
  words are a pure function of the uuid, so the server stores nothing until a result arrives.
  Any instance can verify any token.
* **Single-use tokens.** A result's primary key is the test uuid.
* **Fast leaderboards.** A `leaderboard` table keeps each name's best per mode. One SQL
  statement inserts the result and updates that row. A covering index makes a leaderboard
  read an index-only scan.

## Development

```bash
cargo test
```

```bash
cargo clippy --all-targets -- -D warnings
```
