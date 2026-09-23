-- Remove duplicatas que já existem (mesmo torrent inserido 2x, sem índice
-- único até agora), mantendo a linha mais recente de cada (watch_id, source_item_id).
DELETE FROM episodes
WHERE source_item_id IS NOT NULL
  AND id NOT IN (
    SELECT MAX(id) FROM episodes
    WHERE source_item_id IS NOT NULL
    GROUP BY watch_id, source_item_id
  );

CREATE UNIQUE INDEX idx_episodes_watch_source_unique
  ON episodes(watch_id, source_item_id)
  WHERE source_item_id IS NOT NULL;
