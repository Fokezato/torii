-- Agrupa temporadas pelo anime de verdade (1ª temporada da franquia na
-- AniList, ver sources::anilist::franchise_seasons) em vez de pelo título —
-- temporada sem "Season N" no nome (ex. "Demon Slayer ... Entertainment
-- District Arc") ficava como outro anime na Biblioteca.
ALTER TABLE watches ADD COLUMN series_anilist_id INTEGER;
ALTER TABLE watches ADD COLUMN series_title TEXT;
