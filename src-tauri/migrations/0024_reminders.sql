ALTER TABLE episodes ADD COLUMN watch_reminded_at TEXT;
ALTER TABLE episodes ADD COLUMN delete_warned_at TEXT;

CREATE TABLE airing_notices (
  anilist_id INTEGER NOT NULL,
  episode    INTEGER NOT NULL,
  airing_at  INTEGER NOT NULL,
  notified   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (anilist_id, episode)
);
