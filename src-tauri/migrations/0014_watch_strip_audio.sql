-- "Remover áudios extras" por anime (ver src/postprocess.rs). A opção
-- global em Config, quando ligada, vale pra todos por cima disso.
ALTER TABLE watches ADD COLUMN strip_audio INTEGER NOT NULL DEFAULT 0;
