-- PrismQDA project file schema: the step from version 3 to version 4.
--
-- Run on a version 3 file to bring it to version 4. A new project gets version 1, then steps 2 and 3, then this step,
-- so every project of the current version has the same tables whether it was made new or upgraded.
--
-- Saved queries (Explore, Query Builder): a research question kept under a name.
--   draft_json   The query as the interface builds it, so it can be opened and changed again.
--   spec_json    The same query as the engine runs it.
--   frozen_json  Set when the query is frozen: the codings it found at that moment, so the answer stops changing as
--                the coding grows. A query that is not frozen is run again each time it is opened.
CREATE TABLE saved_query (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(trim(name)) > 0),
    note        TEXT NOT NULL DEFAULT '',
    draft_json  TEXT NOT NULL CHECK (json_valid(draft_json)),
    spec_json   TEXT NOT NULL CHECK (json_valid(spec_json)),
    frozen_at   TEXT,
    frozen_json TEXT CHECK (frozen_json IS NULL OR json_valid(frozen_json)),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX saved_query_name ON saved_query (lower(name));

-- Concept maps (Explore): codes, cases, memos, and free notes placed on a page and joined by named links.
--   map_node.kind   `code`, `case`, or `memo` point at that thing by ref_id (the label follows its name when shown);
--                   `note` is free text kept in label.
--   map_node.x,y    Where the node sits on the page, in the map's own units.
--   map_link        A link between two nodes of the same map, with the words that name the relation.
CREATE TABLE concept_map (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(trim(name)) > 0),
    notes       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX concept_map_name ON concept_map (lower(name));

CREATE TABLE map_node (
    id          INTEGER PRIMARY KEY,
    map_id      INTEGER NOT NULL REFERENCES concept_map (id),
    kind        TEXT NOT NULL CHECK (kind IN ('code', 'case', 'memo', 'note')),
    ref_id      INTEGER,
    label       TEXT NOT NULL DEFAULT '',
    x           REAL NOT NULL DEFAULT 0,
    y           REAL NOT NULL DEFAULT 0,
    CHECK ((kind = 'note' AND ref_id IS NULL) OR (kind <> 'note' AND ref_id IS NOT NULL))
);
CREATE INDEX map_node_map ON map_node (map_id);

CREATE TABLE map_link (
    id          INTEGER PRIMARY KEY,
    map_id      INTEGER NOT NULL REFERENCES concept_map (id),
    from_node   INTEGER NOT NULL REFERENCES map_node (id),
    to_node     INTEGER NOT NULL REFERENCES map_node (id),
    label       TEXT NOT NULL DEFAULT '',
    directed    INTEGER NOT NULL DEFAULT 1 CHECK (directed IN (0, 1)),
    CHECK (from_node <> to_node)
);
CREATE INDEX map_link_map ON map_link (map_id);
