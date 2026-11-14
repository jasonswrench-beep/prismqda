-- Version 9: the Statistics notebook kept in the project. Each page is an analysis (the request that was run, the result it gave, when,
-- and the author's note on it) or a page of notes. An analysis can be run again after the data change; what the page said before is
-- kept as an earlier run, so that the old and the new results can be compared.
--
-- A request is stored as the interface sent it (the procedure, the variables by key, and the options), and a result as the engine
-- returned it, both as JSON, so a page can be read, exported, or run again without the screen that made it.

CREATE TABLE stat_page (
    id            INTEGER PRIMARY KEY,
    position      INTEGER NOT NULL,                                    -- the order of the pages in the notebook
    kind          TEXT NOT NULL CHECK (kind IN ('analysis', 'note')),
    section       TEXT NOT NULL DEFAULT '',                            -- the section (tab) the page is in, or none
    title         TEXT NOT NULL DEFAULT '',                            -- the title of a page of notes
    request_json  TEXT,                                                -- an analysis: what was asked
    result_json   TEXT,                                                -- an analysis: what the engine gave the last time it ran
    ran_at        TEXT,                                                -- an analysis: when it last ran
    note          TEXT NOT NULL DEFAULT '',                            -- the author's note on an analysis, or the body of a page of notes
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    CHECK (kind = 'note' OR (request_json IS NOT NULL AND result_json IS NOT NULL AND ran_at IS NOT NULL))
);

-- What an analysis said before it was run again (the most recent runs; older ones are dropped).
CREATE TABLE stat_page_run (
    id            INTEGER PRIMARY KEY,
    page_id       INTEGER NOT NULL REFERENCES stat_page (id),
    ran_at        TEXT NOT NULL,
    result_json   TEXT NOT NULL
);

CREATE INDEX stat_page_run_by_page ON stat_page_run (page_id, id);
