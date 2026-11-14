-- Version 11: PDF pages. A PDF can be kept inside the project exactly as it was, and an area of one of its pages can be coded.
--
-- The PDF is still an ordinary row of `source` whose `text` is the words read from the file (so search, word coding, and everything that
-- reads text work as before, and the pages the words came from are in its format runs). What is new is the file itself, byte for
-- byte, in `pdf_file`, and the size of each page in `pdf_page`, so a page can be drawn when it is looked at and a person can draw a
-- rectangle on it. A document that was imported before version 11 has no file kept; the original can be attached to it afterwards.
--
-- A coded area of a page is a row of `region`, as the area of a picture is; `page` says which page of the PDF. A picture has one
-- "page", the first. Every coordinate is still a fraction of the page as it is drawn (a page the file turns is drawn upright), so an area
-- keeps its place whatever size the page is drawn at.

CREATE TABLE pdf_file (
    source_id           INTEGER PRIMARY KEY REFERENCES source (id),
    file_name           TEXT NOT NULL,
    page_count          INTEGER NOT NULL CHECK (page_count > 0),
    size_bytes          INTEGER NOT NULL,
    data                BLOB NOT NULL,                          -- the file as it was, unchanged
    created_at          TEXT NOT NULL
);

CREATE TABLE pdf_page (
    source_id           INTEGER NOT NULL REFERENCES pdf_file (source_id),
    page                INTEGER NOT NULL CHECK (page >= 1),
    width               REAL NOT NULL CHECK (width > 0),        -- points (1/72 inch), as the page is drawn
    height              REAL NOT NULL CHECK (height > 0),
    PRIMARY KEY (source_id, page)
);

ALTER TABLE region ADD COLUMN page INTEGER NOT NULL DEFAULT 1 CHECK (page >= 1);
CREATE INDEX region_page ON region (source_id, page);
