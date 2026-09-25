-- Abertura/encerramento "mistos" do AniSkip (créditos por cima de cenas do
-- episódio): mostram botão/marcação mas não pulam sozinhos.
ALTER TABLE skip_segments ADD COLUMN intro_mixed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE skip_segments ADD COLUMN ending_mixed INTEGER NOT NULL DEFAULT 0;

-- Cache antigo foi montado sem pedir os tipos mistos — busca de novo.
DELETE FROM skip_segments;

-- Trechos detectados localmente (capítulos do arquivo ou comparação do
-- áudio entre episódios da mesma temporada, ver src/intro_detect.rs).
-- Completa o que o AniSkip não tem. Linha com tudo NULL = analisado, nada
-- achado. `file_path` = arquivo analisado; se o episódio trocar de arquivo
-- (outra fonte), analisa de novo.
CREATE TABLE detected_segments (
  watch_id        INTEGER NOT NULL REFERENCES watches(id) ON DELETE CASCADE,
  episode_number  INTEGER NOT NULL,
  intro_start_ms  INTEGER,
  intro_end_ms    INTEGER,
  ending_start_ms INTEGER,
  ending_end_ms   INTEGER,
  source          TEXT,
  file_path       TEXT NOT NULL,
  analyzed_at     TEXT NOT NULL,
  PRIMARY KEY (watch_id, episode_number)
);
