-- Version 15: deciding where coders differ on an area of a picture or a PDF page, or a stretch of a recording.
--
-- Adjudication (version 8) settles a unit of words: the coders gave it different codes, and a person says which codes it carries. An area or a
-- stretch has no unit, so what is settled is whether a coded area or stretch belongs in the final layer: coders' areas of one code on one page
-- that overlap (by at least half) are the same area; where every coder who coded the document marked it, the coders agree, and where some did
-- not, a person decides, by accepting it or leaving it out, and says why. One decision stands for an area at a time, kept on the lowest-numbered
-- area of the group, and deciding again replaces it (the log keeps what it replaced). Nothing a coder coded is changed.

CREATE TABLE region_decision (
    region_id           INTEGER PRIMARY KEY REFERENCES region (id) ON DELETE CASCADE,
    adjudicator_id      INTEGER NOT NULL REFERENCES coder (id),
    accepted            INTEGER NOT NULL CHECK (accepted IN (0, 1)),
    reason              TEXT NOT NULL CHECK (length(trim(reason)) > 0),   -- a decision always says why
    created_at          TEXT NOT NULL
);
