-- Player nativo passa a ser o padrão. O modo "external" existia só como
-- configuração salva (nenhum código usava) — todo mundo volta pro nativo;
-- quem quiser o externo escolhe de novo em Config > Reprodução.
UPDATE settings SET value = 'native' WHERE key = 'player_mode';
-- Caminho do VLC não é mais usado (externo = player padrão do Windows).
DELETE FROM settings WHERE key = 'vlc_path';
