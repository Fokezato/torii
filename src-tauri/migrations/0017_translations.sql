-- Cache de traduções automáticas (sinopses da AniList, que só existem em
-- inglês). Chave = texto original + idioma de destino.
CREATE TABLE translations (
  source TEXT NOT NULL,
  target TEXT NOT NULL,
  translated TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (source, target)
);
