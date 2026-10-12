-- Speed and errors per second of each result, in the JSON shape of `Vec<scoring::TimelinePoint>`.
-- Rows stored before this column existed get an empty timeline; new rows always set it.
ALTER TABLE results ADD COLUMN timeline JSONB NOT NULL DEFAULT '[]';
ALTER TABLE results ALTER COLUMN timeline DROP DEFAULT;
