-- "Reduzir resolução" (beta, ver src/downscale.rs).
-- Por anime: NULL = segue a opção global; "original" / "720p".
ALTER TABLE watches ADD COLUMN max_resolution TEXT;
-- Quando o vídeo do episódio já passou pela redução — não reprocessa.
ALTER TABLE episodes ADD COLUMN video_processed_at TEXT;
