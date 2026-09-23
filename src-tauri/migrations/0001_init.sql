CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT
);

CREATE TABLE watches (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  title             TEXT NOT NULL,
  query             TEXT NOT NULL,
  anilist_id        INTEGER,
  cover_url         TEXT,
  quality           TEXT NOT NULL DEFAULT '1080p',
  audio_lang        TEXT,
  sub_lang          TEXT,
  folder            TEXT NOT NULL,
  delete_after_days INTEGER,
  rating            INTEGER,
  active            INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL
);

CREATE TABLE seen_items (
  source_item_id TEXT NOT NULL,
  watch_id       INTEGER NOT NULL REFERENCES watches(id) ON DELETE CASCADE,
  title          TEXT,
  matched        INTEGER NOT NULL DEFAULT 0,
  seen_at        TEXT NOT NULL,
  PRIMARY KEY (source_item_id, watch_id)
);

CREATE TABLE episodes (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  watch_id         INTEGER NOT NULL REFERENCES watches(id) ON DELETE CASCADE,
  source_item_id   TEXT,
  name             TEXT,
  magnet_uri       TEXT,
  info_hash        TEXT,
  save_path        TEXT,
  status           TEXT NOT NULL DEFAULT 'downloading',
  error_message    TEXT,
  jellyfin_item_id TEXT,
  item_path        TEXT,
  added_at         TEXT NOT NULL,
  available_at     TEXT,
  deleted_at       TEXT
);

CREATE INDEX idx_episodes_watch_id  ON episodes(watch_id);
CREATE INDEX idx_episodes_info_hash ON episodes(info_hash);
