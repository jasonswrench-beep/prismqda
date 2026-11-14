-- Version 12: coding a stretch of a recording. A recording can be a document of its own, imported without captions, and a stretch of
-- a recording (from one moment to another) can be given a code as an area of a picture is.
--
-- A stretch is a row of `region` whose shape is `span`. For a span the numbers that were the box around an area are times in
-- milliseconds from the start of the recording: `x0` is where the stretch starts, `x1` is where it ends, and `y0` and `y1` are always
-- 0 and 1 so that stretches of one recording sort by their start. `page` is always 1. A recording is not copied into the project (see
-- version 6), so a stretch is a pair of times, and it keeps its place if the recording is linked again from somewhere else.
--
-- `region` is rebuilt because a column's CHECK cannot be changed in place: the old table only allowed the three shapes of a picture and
-- boxes inside 0 to 1. Every row is copied unchanged. Nothing else refers to `region` by a foreign key.
--
-- `media.stands_alone` is 1 for a recording that was imported without captions: its document's text is a description of the recording
-- (as a picture's text is its description), and the recording is the thing coded. A recording linked to a transcript is 0.

CREATE TABLE region_v12 (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER NOT NULL REFERENCES source (id),
    page                INTEGER NOT NULL DEFAULT 1 CHECK (page >= 1),
    shape               TEXT NOT NULL CHECK (shape IN ('rectangle', 'ellipse', 'polygon', 'span')),
    geometry_json       TEXT NOT NULL CHECK (json_valid(geometry_json)),
    x0                  REAL NOT NULL,
    y0                  REAL NOT NULL,
    x1                  REAL NOT NULL,
    y1                  REAL NOT NULL,
    code_id             INTEGER NOT NULL REFERENCES code (id),
    coder_id            INTEGER NOT NULL REFERENCES coder (id),
    origin              TEXT NOT NULL DEFAULT 'human' CHECK (origin IN ('human', 'ai_suggestion_accepted', 'ai_applied')),
    layer               TEXT NOT NULL DEFAULT 'coder' CHECK (layer IN ('coder', 'final')),
    rationale           TEXT NOT NULL DEFAULT '',
    rating              INTEGER CHECK (rating IS NULL OR rating BETWEEN 1 AND 5),
    created_at          TEXT NOT NULL,
    CHECK (
        (shape = 'span' AND x0 >= 0 AND x1 > x0 AND y0 = 0 AND y1 = 1)
        OR (shape <> 'span' AND x0 >= 0 AND y0 >= 0 AND x1 <= 1 AND y1 <= 1 AND x1 > x0 AND y1 > y0)
    )
);

INSERT INTO region_v12 (id, source_id, page, shape, geometry_json, x0, y0, x1, y1, code_id, coder_id, origin, layer, rationale, rating, created_at)
SELECT id, source_id, page, shape, geometry_json, x0, y0, x1, y1, code_id, coder_id, origin, layer, rationale, rating, created_at FROM region;

DROP TABLE region;
ALTER TABLE region_v12 RENAME TO region;

CREATE INDEX region_source ON region (source_id);
CREATE INDEX region_code ON region (code_id);
CREATE INDEX region_coder ON region (coder_id);
CREATE INDEX region_page ON region (source_id, page);

ALTER TABLE media ADD COLUMN stands_alone INTEGER NOT NULL DEFAULT 0 CHECK (stands_alone IN (0, 1));
