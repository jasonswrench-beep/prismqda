-- PrismQDA project file schema: the step from version 1 to version 2.
--
-- Run on a version 1 file to bring it to version 2. A new project gets version 1 and then this step,
-- so every project of the current version has the same tables whether it was made new or upgraded.
--
-- A coding can carry a rating from 1 to 5 (the Segment Details panel shows it as stars). NULL means
-- "not rated". It is a different state from every number, so a missing rating is never counted as 0.
ALTER TABLE coding ADD COLUMN rating INTEGER CHECK (rating IS NULL OR rating BETWEEN 1 AND 5);
