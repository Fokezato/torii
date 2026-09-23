-- A unicidade da 0006 é por source_item_id — evita a MESMA release entrar
-- 2x, mas não evita 2 releases DIFERENTES do mesmo episódio entrando em
-- polls separados (ex.: poll 1 escolhe ToonsHub como primary pra E04, poll 2
-- — depois de seen_items limpo ou reprocessamento — escolhe VARYG; nenhum dos
-- 2 colide por source_item_id, e o mesmo episódio vira 2 linhas). Guarda o
-- número extraído do título no insert e força 1 linha por (watch, episódio).
ALTER TABLE episodes ADD COLUMN episode_number INTEGER;

CREATE UNIQUE INDEX idx_episodes_watch_episode_number_unique
    ON episodes(watch_id, episode_number)
    WHERE episode_number IS NOT NULL;
