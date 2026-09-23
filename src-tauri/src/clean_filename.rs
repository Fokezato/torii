use regex::Regex;

const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn sanitize(name: &str) -> String {
    name.chars().filter(|c| !FORBIDDEN_CHARS.contains(c)).collect()
}

/// Palavras de qualidade/codec/fonte/grupo que não têm nada a ver com o
/// título do episódio — assim que uma bate, para de coletar palavras pro
/// título. Cada uma é o token INTEIRO após separar por ponto/underscore/
/// espaço (por isso "h.264" vira "h" + "264" e precisa das duas entradas).
const JUNK_TOKEN: &str = r"(?ix)^(
    1080p|720p|480p|2160p|4k|
    web-?dl|webrip|bluray|bdrip|brrip|hdtv|
    hevc|x264|x265|h|264|265|10bit|8bit|
    aac\d?|ddp?\d?|dts|flac|
    multi|dual|dual-?audio|multi-?subs?|msubs?|subs?|
    cr|nf|amzn|bili|abema|hidive|v\d+
)$";

/// Deriva um nome de arquivo limpo tipo "Nome do Anime S03E01 Título do
/// Episódio.mkv" a partir do nome cru de release (cheio de tag de
/// qualidade/codec/grupo). Devolve `None` se não achar um "SxxEyy" no nome
/// cru — nesse caso quem chama deve manter o nome original em vez de
/// arriscar um resultado sem sentido.
pub fn clean_episode_filename(base_title: &str, raw_filename: &str) -> Option<String> {
    let (stem, ext) = raw_filename.rsplit_once('.').unwrap_or((raw_filename, "mkv"));

    let se_re = Regex::new(r"[Ss](\d{1,2})[Ee](\d{1,3})").ok()?;
    let se_match = se_re.find(stem)?;
    let season_episode = se_match.as_str().to_uppercase();

    let junk_re = Regex::new(JUNK_TOKEN).ok()?;
    let after = &stem[se_match.end()..];
    let is_delim = |c: char| c == '.' || c == '_' || c.is_whitespace() || "[](){}".contains(c);
    let title_words: Vec<&str> = after
        .split(is_delim)
        .filter(|s| !s.is_empty())
        .take_while(|w| !junk_re.is_match(w))
        .collect();

    let mut clean = format!("{} {season_episode}", sanitize(base_title).trim());
    if !title_words.is_empty() {
        clean.push(' ');
        clean.push_str(&title_words.join(" "));
    }
    Some(format!("{clean}.{}", ext.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_quality_codec_and_group_tags() {
        // Nomes reais de arquivo baixados nessa sessão (não títulos de RSS —
        // o arquivo de verdade dentro do torrent, que é o que precisa limpar).
        let cases = [
            (
                "Mushoku.Tensei.Jobless.Reincarnation.S03E01.Burn.Bright.Mad.Dog.1080p.CR.WEB-DL.MULTi.AAC2.0.H.264.MSubs-ToonsHub.mkv",
                "Mushoku Tensei Jobless Reincarnation S03E01 Burn Bright Mad Dog.mkv",
            ),
            (
                "Mushoku.Tensei.Jobless.Reincarnation.S03E10.1080p.CR.WEB-DL.MULTi.AAC2.0.H.264.MSubs-ToonsHub.mkv",
                "Mushoku Tensei Jobless Reincarnation S03E10.mkv",
            ),
            (
                "Mushoku.Tensei.Jobless.Reincarnation.S03E04.A.King-Class.Water.Mage.1080p.CR.WEB-DL.MULTi.AAC2.0.H.264.MSubs-ToonsHub.mkv",
                "Mushoku Tensei Jobless Reincarnation S03E04 A King-Class Water Mage.mkv",
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                clean_episode_filename("Mushoku Tensei Jobless Reincarnation", raw),
                Some(expected.to_string()),
                "input: {raw}"
            );
        }
    }

    #[test]
    fn sanitizes_forbidden_filename_characters_in_show_title() {
        let clean = clean_episode_filename(
            "Mushoku Tensei: Jobless Reincarnation",
            "Show.S01E02.Title.1080p.mkv",
        )
        .unwrap();
        assert!(!clean.contains(':'), "colon must be stripped: {clean}");
        assert_eq!(clean, "Mushoku Tensei Jobless Reincarnation S01E02 Title.mkv");
    }

    #[test]
    fn returns_none_when_no_season_episode_pattern_found() {
        assert_eq!(clean_episode_filename("Show", "random_movie_rip_2024.mkv"), None);
    }

    #[test]
    fn preserves_extension_case_insensitively_lowercased() {
        let clean = clean_episode_filename("Show", "Show.S01E01.1080p.MKV").unwrap();
        assert!(clean.ends_with(".mkv"));
    }
}
