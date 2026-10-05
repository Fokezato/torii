-- Chapters named "Intro" were taken as the opening even when they were the cold open.
-- Re-run detection for every episode whose result came from chapters.
DELETE FROM detected_segments WHERE source LIKE 'chapters%';
