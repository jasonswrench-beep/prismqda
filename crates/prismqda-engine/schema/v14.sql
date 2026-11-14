-- Version 14: where a theme stands in its development.
--
-- A theme is a code that groups other codes (`code.kind = 'theme'`, since format 1). Developing themes is a process: a theme begins as a
-- candidate (a grouping of codes that might hang together), is reviewed against the coded passages and the whole of the data, and is
-- finally defined and named. `theme_stage` says which of the three a theme has reached:
--   candidate   a grouping that has been proposed and not yet checked
--   reviewed    checked against the passages of its codes and against the data as a whole
--   defined     named and defined, ready to be written up
-- It means nothing for a code that is not a theme and stays at its default there.

ALTER TABLE code ADD COLUMN theme_stage TEXT NOT NULL DEFAULT 'candidate' CHECK (theme_stage IN ('candidate', 'reviewed', 'defined'));
