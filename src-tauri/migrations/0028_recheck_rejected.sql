-- Releases whose page failed to load (e.g. rate limiting) were wrongly rejected as
-- "language missing" and never checked again. Re-evaluate every rejected release once.
DELETE FROM seen_items WHERE matched = 0;
