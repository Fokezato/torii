-- Modo "Streaming" do anime: nada baixa sozinho. O poller só acha a fonte
-- (episódio fica 'ready'); o download começa quando o episódio é aberto no
-- player (assiste enquanto baixa) e o arquivo é apagado depois de assistido.
ALTER TABLE watches ADD COLUMN streaming INTEGER NOT NULL DEFAULT 0;
