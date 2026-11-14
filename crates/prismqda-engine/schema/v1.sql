-- PrismQDA project file schema, version 1.
--
-- A PrismQDA project is one SQLite file. This file is published so a project can be read
-- with R, Python, or any SQLite tool without PrismQDA installed (specification section 4).
--
-- Conventions
--   * The schema version is stored in the file as PRAGMA user_version.
--   * The file is marked as a PrismQDA project with PRAGMA application_id.
--   * Timestamps are UTC, ISO 8601 text with milliseconds, for example 2026-11-04T15:30:00.123Z.
--   * Text offsets (start_offset, end_offset) count Unicode scalar values (code points) into
--     source.text, which is stored in Unicode NFC. start is inclusive, end is exclusive.
--   * Columns named *_json hold JSON text and are checked with json_valid().
--   * The event table is append-only: triggers refuse any UPDATE or DELETE on it.

CREATE TABLE project (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    project_uid         TEXT NOT NULL,                 -- shared by every copy of one project (merge, section 18.2)
    name                TEXT NOT NULL,
    method_template     TEXT NOT NULL DEFAULT 'free',  -- free, reflexive_ta, content_analysis, grounded_theory, ipa, narrative
    research_questions  TEXT NOT NULL DEFAULT '',
    coding_mode         TEXT NOT NULL DEFAULT 'free_span' CHECK (coding_mode IN ('free_span', 'unit')),
    agreement_target    REAL NOT NULL DEFAULT 0.80,    -- Krippendorff's alpha target (section 18.3)
    offline_lock        INTEGER NOT NULL DEFAULT 0 CHECK (offline_lock IN (0, 1)),   -- section 27
    created_at          TEXT NOT NULL
);

CREATE TABLE source (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    source_type         TEXT NOT NULL CHECK (source_type IN ('document', 'pdf', 'survey', 'transcript', 'social', 'other')),
    folder              TEXT NOT NULL DEFAULT '',
    text                TEXT NOT NULL,                 -- Unicode NFC
    char_count          INTEGER NOT NULL CHECK (char_count > 0),   -- Unicode scalar values in text, kept so offset checks are instant
    formatting_runs_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(formatting_runs_json)),
    original_file_hash  TEXT,                          -- SHA-256 of the imported file, lowercase hex
    import_settings_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(import_settings_json)),
    import_report_json  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(import_report_json)),
    default_unit_kind   TEXT NOT NULL DEFAULT 'sentence'
                        CHECK (default_unit_kind IN ('sentence', 'paragraph', 'speaker_turn', 'survey_response', 'post', 'comment')),
    text_revision       INTEGER NOT NULL DEFAULT 1,    -- increments on every text edit (section 6.1)
    do_not_share        INTEGER NOT NULL DEFAULT 0 CHECK (do_not_share IN (0, 1)),   -- section 27
    created_at          TEXT NOT NULL
);
CREATE INDEX source_original_file_hash ON source (original_file_hash);

CREATE TABLE unit (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER NOT NULL REFERENCES source (id),
    kind                TEXT NOT NULL CHECK (kind IN ('sentence', 'paragraph', 'speaker_turn', 'survey_response', 'post', 'comment')),
    start_offset        INTEGER NOT NULL,
    end_offset          INTEGER NOT NULL,
    speaker             TEXT,
    parent_unit_id      INTEGER REFERENCES unit (id),  -- replies in threaded data
    excluded            INTEGER NOT NULL DEFAULT 0 CHECK (excluded IN (0, 1)),       -- for example interviewer turns
    do_not_share        INTEGER NOT NULL DEFAULT 0 CHECK (do_not_share IN (0, 1)),
    CHECK (start_offset >= 0 AND end_offset > start_offset)
);
CREATE INDEX unit_source ON unit (source_id, start_offset);

CREATE TABLE research_case (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    notes               TEXT NOT NULL DEFAULT ''
);

CREATE TABLE case_member (                              -- a case holds sources, units, or both
    case_id             INTEGER NOT NULL REFERENCES research_case (id),
    source_id           INTEGER REFERENCES source (id),
    unit_id             INTEGER REFERENCES unit (id),
    CHECK ((source_id IS NULL) <> (unit_id IS NULL))   -- exactly one of the two
);

CREATE TABLE attribute (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    applies_to          TEXT NOT NULL CHECK (applies_to IN ('case', 'source', 'unit')),
    level               TEXT NOT NULL CHECK (level IN ('nominal', 'ordinal', 'interval', 'ratio', 'date', 'text')),
    value_labels_json   TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(value_labels_json)),
    missing_codes_json  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(missing_codes_json) AND json_array_length(missing_codes_json) <= 3),
    derived_json        TEXT CHECK (derived_json IS NULL OR json_valid(derived_json)),   -- live derived attributes (section 7.3)
    UNIQUE (applies_to, name)
);

CREATE TABLE attribute_value (
    attribute_id        INTEGER NOT NULL REFERENCES attribute (id),
    target_id           INTEGER NOT NULL,              -- a case, source, or unit id, as attribute.applies_to says
    value               TEXT,
    PRIMARY KEY (attribute_id, target_id)
);

CREATE TABLE code (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    parent_id           INTEGER REFERENCES code (id),
    kind                TEXT NOT NULL DEFAULT 'code' CHECK (kind IN ('code', 'theme')),   -- themes group other codes
    color               TEXT NOT NULL DEFAULT '#808080',
    definition          TEXT NOT NULL DEFAULT '',
    inclusion_criteria  TEXT NOT NULL DEFAULT '',
    exclusion_criteria  TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'merged', 'retired')),
    merged_into_id      INTEGER REFERENCES code (id),
    version             INTEGER NOT NULL DEFAULT 1,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    do_not_share        INTEGER NOT NULL DEFAULT 0 CHECK (do_not_share IN (0, 1)),
    created_at          TEXT NOT NULL
);
CREATE INDEX code_parent ON code (parent_id);

CREATE TABLE code_set (                                 -- groups codes across branches without changing the tree (section 9.1)
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL UNIQUE,
    description         TEXT NOT NULL DEFAULT ''
);

CREATE TABLE code_set_member (
    code_set_id         INTEGER NOT NULL REFERENCES code_set (id),
    code_id             INTEGER NOT NULL REFERENCES code (id),
    PRIMARY KEY (code_set_id, code_id)
);

CREATE TABLE coder (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    role                TEXT NOT NULL CHECK (role IN ('human', 'model')),
    local_id            TEXT NOT NULL,                 -- a name plus a local id (section 18.1)
    provider            TEXT,                          -- model coders only
    model_name          TEXT,
    model_version       TEXT,
    settings_json       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings_json)),
    created_at          TEXT NOT NULL,
    UNIQUE (local_id)
);

CREATE TABLE ai_run (
    id                  INTEGER PRIMARY KEY,
    route               TEXT NOT NULL CHECK (route IN ('packet', 'local', 'mcp', 'remote')),
    model               TEXT NOT NULL DEFAULT '',
    settings_json       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings_json)),
    prompt              TEXT NOT NULL,                 -- the exact text sent
    prompt_hash         TEXT NOT NULL,                 -- SHA-256 of prompt
    units_sent_json     TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(units_sent_json)),
    raw_output          TEXT NOT NULL DEFAULT '',
    parsed_result_json  TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(parsed_result_json)),
    accepted_count      INTEGER NOT NULL DEFAULT 0,
    rejected_count      INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL
);

CREATE TABLE coding (
    id                  INTEGER PRIMARY KEY,
    source_id           INTEGER NOT NULL REFERENCES source (id),
    start_offset        INTEGER NOT NULL,
    end_offset          INTEGER NOT NULL,
    code_id             INTEGER NOT NULL REFERENCES code (id),
    coder_id            INTEGER NOT NULL REFERENCES coder (id),
    origin              TEXT NOT NULL DEFAULT 'human' CHECK (origin IN ('human', 'ai_suggestion_accepted', 'ai_applied')),
    layer               TEXT NOT NULL DEFAULT 'coder' CHECK (layer IN ('coder', 'final')),   -- 'final' is the adjudicated layer (section 18.4)
    rationale           TEXT NOT NULL DEFAULT '',
    unit_id             INTEGER REFERENCES unit (id),
    ai_run_id           INTEGER REFERENCES ai_run (id),
    created_at          TEXT NOT NULL,
    CHECK (start_offset >= 0 AND end_offset > start_offset)
);
CREATE INDEX coding_source ON coding (source_id, start_offset);
CREATE INDEX coding_code ON coding (code_id);
CREATE INDEX coding_coder ON coding (coder_id);

CREATE TABLE proposal (                                 -- the review queue: autocoding and AI suggestions wait here (sections 7.5, 10)
    id                  INTEGER PRIMARY KEY,
    kind                TEXT NOT NULL CHECK (kind IN ('coding')),
    origin              TEXT NOT NULL CHECK (origin IN ('autocode', 'ai')),
    batch               TEXT NOT NULL,                 -- groups the proposals one search or one AI run made
    payload_json        TEXT NOT NULL CHECK (json_valid(payload_json)),
    status              TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
    ai_run_id           INTEGER REFERENCES ai_run (id),
    created_at          TEXT NOT NULL,
    decided_at          TEXT,
    decided_by          TEXT
);
CREATE INDEX proposal_status ON proposal (status, batch);

CREATE TABLE phase_progress (                           -- method template checklist state (section 24.1)
    phase_id            TEXT NOT NULL,
    item_index          INTEGER NOT NULL,
    done                INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
    done_at             TEXT,
    PRIMARY KEY (phase_id, item_index)
);

CREATE TABLE code_anchor (                              -- anchor examples for a code
    code_id             INTEGER NOT NULL REFERENCES code (id),
    coding_id           INTEGER NOT NULL REFERENCES coding (id),
    PRIMARY KEY (code_id, coding_id)
);

CREATE TABLE memo (
    id                  INTEGER PRIMARY KEY,
    kind                TEXT NOT NULL CHECK (kind IN ('analytic', 'reflexive', 'method_decision', 'todo')),
    scope               TEXT NOT NULL CHECK (scope IN ('project', 'source', 'code', 'coding', 'case', 'phase', 'standalone')),
    target_id           INTEGER,
    phase               TEXT,
    title               TEXT NOT NULL DEFAULT '',
    body                TEXT NOT NULL DEFAULT '',
    body_format         TEXT NOT NULL DEFAULT 'markdown',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE link (                                     -- a labeled connection between any two entities
    id                  INTEGER PRIMARY KEY,
    from_type           TEXT NOT NULL,
    from_id             INTEGER NOT NULL,
    to_type             TEXT NOT NULL,
    to_id               INTEGER NOT NULL,
    label               TEXT NOT NULL DEFAULT ''
);

CREATE TABLE codebook_version (
    id                  INTEGER PRIMARY KEY,
    version             INTEGER NOT NULL UNIQUE,
    note                TEXT NOT NULL DEFAULT '',
    snapshot_json       TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    created_at          TEXT NOT NULL
);

CREATE TABLE reliability_run (
    id                  INTEGER PRIMARY KEY,
    coders_json         TEXT NOT NULL CHECK (json_valid(coders_json)),
    codes_json          TEXT NOT NULL CHECK (json_valid(codes_json)),
    units_json          TEXT NOT NULL CHECK (json_valid(units_json)),
    coefficients_json   TEXT NOT NULL CHECK (json_valid(coefficients_json)),
    results_json        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(results_json)),
    reporting_term      TEXT NOT NULL DEFAULT 'intercoder_reliability' CHECK (reporting_term IN ('intercoder_reliability', 'concordance')),
    sample_seed         INTEGER,
    report_text         TEXT NOT NULL DEFAULT '',
    archived            INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),   -- units stay locked until archived (section 18.3)
    created_at          TEXT NOT NULL
);

CREATE TABLE analysis_run (
    id                  INTEGER PRIMARY KEY,
    procedure_name      TEXT NOT NULL,
    variables_json      TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variables_json)),
    options_json        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(options_json)),
    result_tables_json  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(result_tables_json)),
    apa_text            TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL
);

CREATE TABLE event (                                    -- the audit trail (sections 4.2 and 27)
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    ts                  TEXT NOT NULL,
    actor               TEXT NOT NULL,
    session             TEXT NOT NULL,
    action              TEXT NOT NULL,                 -- for example coding.add, code.add, source.import, annotation
    target_type         TEXT,
    target_id           INTEGER,
    before_json         TEXT CHECK (before_json IS NULL OR json_valid(before_json)),
    after_json          TEXT CHECK (after_json IS NULL OR json_valid(after_json))
);
CREATE INDEX event_action ON event (action);
CREATE INDEX event_target ON event (target_type, target_id);

CREATE TRIGGER event_no_update BEFORE UPDATE ON event
BEGIN
    SELECT RAISE(ABORT, 'the event log is append-only: entries cannot be changed');
END;

CREATE TRIGGER event_no_delete BEFORE DELETE ON event
BEGIN
    SELECT RAISE(ABORT, 'the event log is append-only: entries cannot be deleted');
END;
