# platen

A typing test typed on a typewriter. The frontend (`web/`, Dioxus compiled to WebAssembly)
draws an isometric typewriter: the test text sits on the sheet in its carriage, each key press
strikes a typebar and moves the carriage, and mistakes print from the red half of the ribbon.
The backend (`server/`, axum + Postgres) issues tests and scores them.

The server issues tests and scores them itself. A client never reports its own WPM: it sends
every keystroke with a timestamp, and the server replays them against the text it issued. This
blocks the easy cheats (posting a fake score, replaying a token, pasting text, scripted typing
at a fixed rhythm).

## Repository layout

```text
.
├── server/              Backend crate: the API, SQL migrations and the word list
├── web/                 Frontend crate: the Dioxus app, its assets and design docs
├── e2e/                 End-to-end checks: Postgres, server, frontend and a real browser
├── scripts/             Generators for committed files: the typewriter drawing, the word list
├── docs/                Guides that span the whole repository, such as deployment
├── Dockerfile           Production image (see docs/deployment.md)
├── render.yaml          Render Blueprint (see docs/deployment.md)
└── docker-compose.yml   Local Postgres for development
```

`server/` and `web/` are separate Cargo projects, each with its own `Cargo.lock`. They build
for different targets (native and WebAssembly) with different release profiles: the server is
optimised for speed and the frontend for download size.

Each directory has its own README:

- `server/README.md`: configuration, the API, scoring and the design notes.
- `web/README.md`: frontend setup, the dev server and the production bundle.
- `e2e/README.md`: what the end-to-end run checks and how to add checks.
- `scripts/README.md`: how to regenerate the typewriter drawing and the word list.
- `docs/deployment.md`: how the app is deployed to Render and Neon.

## Run it

Start Postgres and create the server's `.env`:

```bash
docker compose up -d
```

```bash
cp server/.env.example server/.env
```

Set `TOKEN_SECRET` in `server/.env` to at least 32 random bytes, for example the output of
`openssl rand -base64 48`. Then start the server. Migrations run on startup.

```bash
cd server && cargo run --release
```

To use the app in a browser, run the frontend dev server as `web/README.md` describes.

## Development

Run the checks in `server/` and in `web/`. CI runs the same commands in both.

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Before a branch goes to CI, run the end-to-end checks:

```bash
cd e2e && npm ci && node run.mjs
```
