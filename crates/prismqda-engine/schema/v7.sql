-- Version 7: images. A document can be a picture, kept inside the project file, and an area of the picture can be coded.
--
-- An image document is an ordinary row of `source`: its `text` is the picture's description (the alternative text a screen reader
-- speaks), so everything that reads text still works. The picture itself, byte for byte as the file was, is in `image`.
--
-- A coded area is a row of `region`, kept apart from `coding`: a coding is a stretch of text counted in characters, and an area is a
-- shape counted in fractions of the picture. A row of `region` is one code given to one area of one picture by one coder, as a row of
-- `coding` is one code given to one stretch of one document, so the two are counted side by side.
--
-- Every coordinate is a fraction of the picture as it is shown (turned upright, as a camera's own turn marker says): 0 is the left or
-- top edge, 1 is the right or bottom edge. A picture saved again at another size keeps its areas.

CREATE TABLE image (
    source_id           INTEGER PRIMARY KEY REFERENCES source (id),
    mime                TEXT NOT NULL CHECK (mime IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/bmp')),
    file_name           TEXT NOT NULL,
    width               INTEGER NOT NULL CHECK (width > 0),     -- pixels, as shown (a camera's turn marker is applied)
    height              INTEGER NOT NULL CHECK (height > 0),
    size_bytes          INTEGER NOT NULL,
    data                BLOB NOT NULL,                          -- the file as it was, unchanged
    created_at          TEXT NOT NULL
);

CREATE TABLE region (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER NOT NULL REFERENCES source (id),
    shape               TEXT NOT NULL CHECK (shape IN ('rectangle', 'ellipse', 'polygon')),
    geometry_json       TEXT NOT NULL CHECK (json_valid(geometry_json)),   -- the shape and its numbers, written one way so equal shapes read equal
    x0                  REAL NOT NULL,                          -- the box around the shape
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
    CHECK (x0 >= 0 AND y0 >= 0 AND x1 <= 1 AND y1 <= 1 AND x1 > x0 AND y1 > y0)
);
CREATE INDEX region_source ON region (source_id);
CREATE INDEX region_code ON region (code_id);
CREATE INDEX region_coder ON region (coder_id);
