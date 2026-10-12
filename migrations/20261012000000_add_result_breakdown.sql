-- Per-key and per-word mistakes of each result, in the JSON shape of `scoring::Breakdown`.
-- Results stored before this column existed get an empty breakdown; new rows must supply one.
ALTER TABLE results
    ADD COLUMN breakdown JSONB NOT NULL DEFAULT '{"keys": [], "missed_words": []}';
ALTER TABLE results ALTER COLUMN breakdown DROP DEFAULT;
