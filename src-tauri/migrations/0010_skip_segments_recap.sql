-- Resumo do episódio anterior ("recap" no AniSkip), além de abertura/encerramento.
ALTER TABLE skip_segments ADD COLUMN recap_start_ms INTEGER;
ALTER TABLE skip_segments ADD COLUMN recap_end_ms INTEGER;

-- Cache antigo foi montado sem perguntar por recap — apaga pra buscar de novo.
DELETE FROM skip_segments;
