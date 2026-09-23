-- Guarda TODOS os releases que casaram com um episódio (não só o baixado),
-- pra permitir trocar de fonte depois sem precisar buscar no Nyaa de novo.
CREATE TABLE episode_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id INTEGER NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    source_item_id TEXT NOT NULL,
    title TEXT NOT NULL,
    magnet_uri TEXT NOT NULL,
    seeders INTEGER,
    leechers INTEGER,
    size TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_episode_sources_episode_id ON episode_sources(episode_id);
CREATE UNIQUE INDEX idx_episode_sources_unique ON episode_sources(episode_id, source_item_id);
