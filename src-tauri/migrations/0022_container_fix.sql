-- MKV gravado pelo ffmpeg (Lavf) traz CRC32 dentro do índice (Cues), e o
-- leitor de MKV do VLC 3 se perde nele: ~10s pra abrir o episódio. O Torii
-- regrava esses arquivos uma vez sem o CRC32 (ver src/mkv_fix.rs); isto
-- marca quem já foi conferido/corrigido.
ALTER TABLE episodes ADD COLUMN container_fixed_at TEXT;
