-- Version 8: adjudication. When coders gave different codes to the same unit, a person settles it with a reason. The decision is kept
-- apart from every coder's own coding: nothing a coder coded is changed, and the decisions together are the final layer.
--
-- One decision stands for a unit at a time. Deciding again replaces it, and the audit log keeps what it replaced. A decision may name no
-- code at all, which says that none of the codes applies.

CREATE TABLE adjudication (
    id                  INTEGER PRIMARY KEY,
    unit_id             INTEGER NOT NULL UNIQUE REFERENCES unit (id),
    adjudicator_id      INTEGER NOT NULL REFERENCES coder (id),
    reason              TEXT NOT NULL CHECK (length(trim(reason)) > 0),   -- a decision always says why
    created_at          TEXT NOT NULL
);

CREATE TABLE adjudication_code (
    adjudication_id     INTEGER NOT NULL REFERENCES adjudication (id),
    code_id             INTEGER NOT NULL REFERENCES code (id),
    PRIMARY KEY (adjudication_id, code_id)
);
