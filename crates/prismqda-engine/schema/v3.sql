-- PrismQDA project file schema: the step from version 2 to version 3.
--
-- Run on a version 2 file to bring it to version 3. A new project gets version 1, then step 2, then this step,
-- so every project of the current version has the same tables whether it was made new or upgraded.
--
-- Code properties (Codebook Editor):
--   exclusive_group  The name of a group of codes that exclude each other: one coder cannot give two codes of
--                    the same group to overlapping words of a source. Empty means the code is in no group.
--                    Names are compared without regard to case.
--   rated            1 when codings of this code can carry a rating (the coding.rating column from version 2).
--   rating_label     What the rating measures, for example "Intensity". Shown beside the stars.
--   rating_low,      What a 1 and a 5 mean for this code. The scale itself is always 1 to 5.
--   rating_high
-- A code whose codings already carry ratings is marked rated, so nothing that was rated stops being rated.
ALTER TABLE code ADD COLUMN exclusive_group TEXT NOT NULL DEFAULT '';
ALTER TABLE code ADD COLUMN rated INTEGER NOT NULL DEFAULT 0 CHECK (rated IN (0, 1));
ALTER TABLE code ADD COLUMN rating_label TEXT NOT NULL DEFAULT '';
ALTER TABLE code ADD COLUMN rating_low TEXT NOT NULL DEFAULT '';
ALTER TABLE code ADD COLUMN rating_high TEXT NOT NULL DEFAULT '';
UPDATE code SET rated = 1 WHERE id IN (SELECT DISTINCT code_id FROM coding WHERE rating IS NOT NULL);

-- A link between a code and a coding is an example of the code (the coding is of that code) or a counterexample
-- (a passage that looks like it belongs to the code but does not, so it is a coding of another code).
ALTER TABLE code_anchor ADD COLUMN kind TEXT NOT NULL DEFAULT 'example' CHECK (kind IN ('example', 'counter'));
