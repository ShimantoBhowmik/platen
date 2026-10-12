-- Every verified result. `id` is the test id from the signed token, so the primary key also
-- makes each token single-use.
CREATE TABLE results (
    id                   UUID PRIMARY KEY,
    -- `TestMode::code()`: 0-3 are time 15/30/60/120 s, 4-7 are words 10/25/50/100.
    mode                 SMALLINT         NOT NULL CHECK (mode BETWEEN 0 AND 7),
    name                 TEXT             CHECK (char_length(name) BETWEEN 1 AND 24),
    wpm                  DOUBLE PRECISION NOT NULL,
    raw_wpm              DOUBLE PRECISION NOT NULL,
    accuracy             DOUBLE PRECISION NOT NULL,
    consistency          DOUBLE PRECISION NOT NULL,
    correct_keystrokes   INTEGER          NOT NULL CHECK (correct_keystrokes >= 0),
    incorrect_keystrokes INTEGER          NOT NULL CHECK (incorrect_keystrokes >= 0),
    elapsed_ms           INTEGER          NOT NULL CHECK (elapsed_ms > 0),
    created_at           TIMESTAMPTZ      NOT NULL
);

-- Each name's best result per mode, kept current on insert. Reading a leaderboard is then a
-- bounded index-only scan instead of a per-request aggregation over all results.
CREATE TABLE leaderboard (
    mode       SMALLINT         NOT NULL,
    name       TEXT             NOT NULL,
    result_id  UUID             NOT NULL REFERENCES results (id),
    wpm        DOUBLE PRECISION NOT NULL,
    accuracy   DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    PRIMARY KEY (mode, name)
);

CREATE INDEX leaderboard_ranking ON leaderboard (mode, wpm DESC, created_at)
    INCLUDE (name, accuracy, result_id);
