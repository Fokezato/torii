-- "Remover áudios extras" (ver src/audio_strip.rs): quando o arquivo do
-- episódio já passou pela limpeza de áudio — não reprocessa.
ALTER TABLE episodes ADD COLUMN audio_processed_at TEXT;
