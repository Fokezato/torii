//! "Remover áudios extras" (beta): tira do arquivo baixado as faixas de
//! áudio fora dos idiomas preferidos do player (Config > Reprodução) —
//! repack costuma trazer 8–10 dublagens, 35–120MB cada. Japonês (original)
//! e faixa sem idioma marcado ficam sempre. Só remux (`-c copy`), sem
//! recodificar: rápido e sem perda de qualidade. Quem chama: `postprocess`.

use crate::ffmpeg;
use crate::media_file::{self, Outcome, ProcessError, ProbeStream};
use std::path::Path;

/// ISO 639-2/1 → nome em inglês minúsculo (mesmo vocabulário das
/// preferências, ex. "Portuguese (Brazil)" → "portuguese").
fn language_name(code: &str) -> Option<&'static str> {
    let base = code.split(['-', '_']).next().unwrap_or("").to_lowercase();
    Some(match base.as_str() {
        "jpn" | "ja" => "japanese",
        "eng" | "en" => "english",
        "por" | "pt" => "portuguese",
        "spa" | "es" => "spanish",
        "fre" | "fra" | "fr" => "french",
        "ger" | "deu" | "de" => "german",
        "ita" | "it" => "italian",
        "rus" | "ru" => "russian",
        "ara" | "ar" => "arabic",
        "chi" | "zho" | "zh" => "chinese",
        "pol" | "pl" => "polish",
        "ind" | "id" => "indonesian",
        "may" | "msa" | "ms" => "malay",
        "tha" | "th" => "thai",
        "vie" | "vi" => "vietnamese",
        "kor" | "ko" => "korean",
        "hin" | "hi" => "hindi",
        "tur" | "tr" => "turkish",
        _ => return None,
    })
}

/// Preferências do jeito que ficam salvas ("Portuguese (Brazil),English")
/// → nomes base minúsculos (["portuguese", "english"]).
pub fn parse_preferred(setting: &str) -> Vec<String> {
    setting
        .split(',')
        .map(|p| p.split('(').next().unwrap_or("").trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

pub fn strip(paths: &ffmpeg::FfmpegPaths, file: &Path, preferred: &[String]) -> Result<Outcome, ProcessError> {
    if !media_file::is_mkv(file) {
        return Ok(Outcome::Nothing);
    }
    let original = media_file::probe(&paths.ffprobe, file).map_err(ProcessError::Permanent)?;

    let keep_audio = |s: &ProbeStream| match s.tags.get("language").and_then(|l| language_name(l)) {
        // Sem idioma reconhecível = não dá pra saber o que é, fica.
        None => true,
        Some("japanese") => true,
        Some(name) => preferred.iter().any(|p| p == name),
    };
    let is_audio = |s: &ProbeStream| s.codec_type.as_deref() == Some("audio");
    let audio_count = original.streams.iter().filter(|s| is_audio(s)).count();
    let kept: Vec<&ProbeStream> = original.streams.iter().filter(|s| is_audio(s) && keep_audio(s)).collect();
    if kept.is_empty() || kept.len() == audio_count {
        return Ok(Outcome::Nothing);
    }

    let mut cmd = ffmpeg::command(&paths.ffmpeg);
    cmd.args(["-nostdin", "-v", "error", "-y", "-i"]).arg(file);
    // Stream por stream, na ordem original, pulando só o áudio descartado —
    // preserva vídeo, legendas e anexos (fontes do ASS).
    for s in &original.streams {
        if is_audio(s) && !keep_audio(s) {
            continue;
        }
        cmd.arg("-map").arg(format!("0:{}", s.index));
    }
    cmd.args(["-c", "copy"]);
    // Se a faixa padrão era uma das removidas, a 1ª que sobrou vira padrão.
    if !kept.iter().any(|s| s.disposition.get("default") == Some(&1)) {
        cmd.args(["-disposition:a:0", "default"]);
    }
    cmd.arg(media_file::temp_path(file));
    media_file::run_and_replace(&paths.ffprobe, cmd, file, &original)
}

#[cfg(test)]
mod tests {
    use super::{language_name, parse_preferred};

    #[test]
    fn language_name_maps_iso_codes_including_region_variants() {
        assert_eq!(language_name("jpn"), Some("japanese"));
        assert_eq!(language_name("por"), Some("portuguese"));
        assert_eq!(language_name("pt-BR"), Some("portuguese"));
        assert_eq!(language_name("SPA"), Some("spanish"));
        assert_eq!(language_name("zh_Hant"), Some("chinese"));
    }

    #[test]
    fn language_name_unknown_or_undefined_is_none() {
        // "und"/vazio = faixa sem idioma marcado — `strip` mantém.
        assert_eq!(language_name("und"), None);
        assert_eq!(language_name(""), None);
    }

    #[test]
    fn parse_preferred_drops_region_and_blanks() {
        assert_eq!(parse_preferred("Portuguese (Brazil),English"), vec!["portuguese", "english"]);
        assert!(parse_preferred("").is_empty());
    }
}
