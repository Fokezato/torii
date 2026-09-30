-- Existing installs already know Torii: skip the first-run welcome for them.
INSERT OR IGNORE INTO settings (key, value)
SELECT 'onboarding_done', '1' WHERE EXISTS (SELECT 1 FROM watches);
