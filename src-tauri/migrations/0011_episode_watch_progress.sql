-- Progresso de reprodução (player nativo) e quando o episódio foi assistido
-- até o fim — base do "apagar depois de assistir" (ver engine::cleanup_once).
ALTER TABLE episodes ADD COLUMN watch_position_ms INTEGER;
ALTER TABLE episodes ADD COLUMN watched_at TEXT;
