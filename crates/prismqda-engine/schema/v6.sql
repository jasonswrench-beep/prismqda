-- Version 6: recordings. A document can have an audio or video recording linked to it, and a unit can say where in the
-- recording it is spoken. PrismQDA keeps where the recording is on this computer, not the recording itself.

CREATE TABLE media (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER NOT NULL UNIQUE REFERENCES source (id),
    kind                TEXT NOT NULL CHECK (kind IN ('audio', 'video')),
    path                TEXT NOT NULL,                 -- the full path of the recording on this computer
    file_name           TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL,
    fingerprint         TEXT NOT NULL,                 -- SHA-256 of the size and the first and last megabyte, to tell whether the file changed
    duration_ms         INTEGER,                       -- the last time any unit of the document names, when it has times
    created_at          TEXT NOT NULL
);

ALTER TABLE unit ADD COLUMN time_start_ms INTEGER;
ALTER TABLE unit ADD COLUMN time_end_ms INTEGER;
