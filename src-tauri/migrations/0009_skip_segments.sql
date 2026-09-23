-- Skip intro/ending (AniSkip, ver src/sources/aniskip.rs). AniSkip é indexado
-- por MyAnimeList id, não AniList id — os 2 nem sempre coincidem, então
-- resolve 1x por watch (via campo idMal da própria AniList) e guarda aqui
-- pra não repetir a chamada de rede em todo episódio.
ALTER TABLE watches ADD COLUMN mal_id INTEGER;

-- Cache por (watch, episódio) — independe de qual release foi baixada
-- (sobrevive re-download/troca de fonte). Linha com todos os *_ms NULL
-- significa "já perguntei pra AniSkip e não tinha nada" — evita bater na
-- API de novo pra sempre nesse episódio.
CREATE TABLE skip_segments (
  watch_id        INTEGER NOT NULL REFERENCES watches(id) ON DELETE CASCADE,
  episode_number  INTEGER NOT NULL,
  intro_start_ms  INTEGER,
  intro_end_ms    INTEGER,
  ending_start_ms INTEGER,
  ending_end_ms   INTEGER,
  fetched_at      TEXT NOT NULL,
  PRIMARY KEY (watch_id, episode_number)
);
