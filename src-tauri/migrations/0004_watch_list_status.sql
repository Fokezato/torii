ALTER TABLE watches ADD COLUMN list_status TEXT NOT NULL DEFAULT 'watching';
ALTER TABLE watches ADD COLUMN episodes INTEGER;
