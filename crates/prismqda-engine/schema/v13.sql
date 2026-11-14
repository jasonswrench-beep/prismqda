-- Version 13: pseudonymizing and redacting. The researcher writes a list of names and other identifiers to find, and what to put in
-- their place; the list can be applied to everything that leaves the project (a switch), or to the project's own text for good.
--
-- `deid_rule` is one thing to find and what replaces it.
--   term         The words to find. A run of spaces in the term matches any run of spaces and line breaks in the text, and a
--                curly apostrophe or quotation mark matches the straight one.
--   replacement  What takes its place (a label such as "Participant 3", or "[redacted]"). Never empty: an empty replacement
--                would delete words and could take a coding with it.
--   kind         `pseudonym` (a name for a name) or `redact` (a mark that says something was taken out). The two behave the same;
--                the kind is for the researcher, for the key file, and for the report.
--   match_case   1 when "Will" must not find "will".
--   whole_word   1 (the default) when "Ann" must not find "Anne" or "Hannah".
--
-- A project that has had its names replaced for good does not keep this list (the list is the key to the names). The researcher is
-- offered the key as a separate file before the list is removed from the project.
--
-- `project.deid_exports` is 1 while the list is applied to everything that leaves the project: Word, Excel, CSV, the report, and
-- the packets taken to a chatbot.

CREATE TABLE deid_rule (
    id                  INTEGER PRIMARY KEY,
    term                TEXT NOT NULL CHECK (length(trim(term)) > 0),
    replacement         TEXT NOT NULL CHECK (length(trim(replacement)) > 0),
    kind                TEXT NOT NULL DEFAULT 'pseudonym' CHECK (kind IN ('pseudonym', 'redact')),
    match_case          INTEGER NOT NULL DEFAULT 0 CHECK (match_case IN (0, 1)),
    whole_word          INTEGER NOT NULL DEFAULT 1 CHECK (whole_word IN (0, 1)),
    note                TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL
);

ALTER TABLE project ADD COLUMN deid_exports INTEGER NOT NULL DEFAULT 0 CHECK (deid_exports IN (0, 1));
