-- Version 16: ids that are never given again.
--
-- A code or a case keeps its number for as long as the project exists, and the number is never given to another code or case afterwards.
-- SQLite hands a number out again when the newest row is removed, and Undo of "add a code" or "add a case" removes it. A file written for
-- another program that names a code or a case by its number (the case table PrismQDA sends to PrismStatistics has a column for each
-- code and a row for each case) could then mean a different code or case from one export to the next, with nothing to show it.
--
-- The table keeps the highest number given for each kind of row. A new code or case takes the number above the higher of this and the
-- highest it holds now (the engine does this where it adds one), so removing the newest row does not free its number. A file brought
-- forward starts from the highest number it holds: a number that was given again before this version cannot be known.

CREATE TABLE id_floor (
    table_name          TEXT PRIMARY KEY CHECK (table_name IN ('code', 'research_case')),
    last_id             INTEGER NOT NULL CHECK (last_id >= 0)
);

INSERT INTO id_floor (table_name, last_id) SELECT 'code', COALESCE(MAX(id), 0) FROM code;
INSERT INTO id_floor (table_name, last_id) SELECT 'research_case', COALESCE(MAX(id), 0) FROM research_case;
