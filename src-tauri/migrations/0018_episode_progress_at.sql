-- Quando o player salvou o progresso pela última vez — ordena o "Continuar
-- assistindo" da Biblioteca. Episódios já assistidos herdam a data de
-- quando foram assistidos.
ALTER TABLE episodes ADD COLUMN watch_progress_at TEXT;
UPDATE episodes SET watch_progress_at = watched_at WHERE watched_at IS NOT NULL;
