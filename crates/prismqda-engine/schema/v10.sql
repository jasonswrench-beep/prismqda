-- Version 10: Mixed Methods kept in the project. Three small things the researcher decides and the program keeps:
--
--   * the design of the study (at most one: convergent, explanatory sequential, exploratory sequential, or embedded), which strand has
--     priority, and the purpose of mixing the two;
--   * the triangulation matrix: a row for each topic with what each strand says in the researcher's words, an optional code and an
--     optional variable of cases that stand for the topic in the data (so the numbers and the words can be shown beside it), and the
--     researcher's mark: convergence, complementarity, or discordance between the strands;
--   * the integration log: dated entries saying where and how the two kinds of data were brought together, and why.

CREATE TABLE mixed_design (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    kind          TEXT NOT NULL CHECK (kind IN ('convergent', 'explanatory_sequential', 'exploratory_sequential', 'embedded')),
    priority      TEXT NOT NULL DEFAULT 'equal' CHECK (priority IN ('equal', 'quantitative', 'qualitative')),
    purpose       TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE triangulation (
    id            INTEGER PRIMARY KEY,
    position      INTEGER NOT NULL,                                    -- the order of the rows in the matrix
    topic         TEXT NOT NULL,                                       -- the researcher's name for the topic
    code_id       INTEGER REFERENCES code (id),                        -- the code whose words are the qualitative evidence, if any
    variable      TEXT NOT NULL DEFAULT '',                            -- the variable of cases (by name) that is the quantitative evidence, if any
    qualitative   TEXT NOT NULL DEFAULT '',                            -- what the qualitative strand says, in the researcher's words
    quantitative  TEXT NOT NULL DEFAULT '',                            -- what the quantitative strand says, in the researcher's words
    mark          TEXT NOT NULL DEFAULT '' CHECK (mark IN ('', 'convergence', 'complementarity', 'discordance')),   -- none until marked
    note          TEXT NOT NULL DEFAULT '',                            -- why the researcher marked it so
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE integration_entry (
    id            INTEGER PRIMARY KEY,
    on_date       TEXT NOT NULL,                                       -- the day it was done, as YYYY-MM-DD
    stage         TEXT NOT NULL CHECK (stage IN ('design', 'data_collection', 'analysis', 'interpretation', 'reporting')),
    how           TEXT NOT NULL CHECK (how IN ('connecting', 'building', 'merging', 'embedding', 'transforming')),
    what          TEXT NOT NULL,                                       -- what was done to bring the two kinds of data together
    why           TEXT NOT NULL DEFAULT '',                            -- why, in the researcher's words
    actor         TEXT NOT NULL DEFAULT '',                            -- who wrote the entry
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX integration_entry_by_date ON integration_entry (on_date, id);
