-- O parser de idioma não lia descrições em bloco ("**Subtitles**" e uma
-- faixa por linha) e rejeitava releases que tinham o idioma
-- pedido. Esquece os candidatos rejeitados pra próxima busca reavaliar com
-- o parser corrigido. Os aceitos (matched = 1) continuam lembrados.
DELETE FROM seen_items WHERE matched = 0;
