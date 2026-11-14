-- PrismQDA project file schema: the step from version 4 to version 5.
--
-- Run on a version 4 file to bring it to version 5. A new project gets version 1, then steps 2 to 4, then this step, so every
-- project of the current version has the same tables whether it was made new or upgraded.
--
-- AI (specification section 19):
--   code.ai_suggestible   1 when an AI suggestion may use this code (the default), 0 when it may not. A code that may not be
--                         suggested is left out of every packet.
--   project.ai_mode       `packet` (the default): the person may build a prompt packet to take to a chatbot of their choice and
--                         paste the answer back, nothing is ever sent by the program; `off`: no packet can be built in this project.
ALTER TABLE code ADD COLUMN ai_suggestible INTEGER NOT NULL DEFAULT 1 CHECK (ai_suggestible IN (0, 1));
ALTER TABLE project ADD COLUMN ai_mode TEXT NOT NULL DEFAULT 'packet' CHECK (ai_mode IN ('off', 'packet'));
