//! "Reduzir resolução" (beta): recodifica o vídeo do episódio baixado pra
//! 720p. Diferente de escolher release de resolução menor — nem todo anime
//! tem. Medido ao vivo (Mushoku Tensei S3, 1080p WEB): vídeo ~10x menor,
//! ~3min/episódio com NVENC, ~7min em CPU (SVT-AV1), sem bloco visível.
//! Quem chama: `postprocess`.

use crate::ffmpeg;
use crate::media_file::{self, Outcome, ProcessError};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    Nvenc,
    Qsv,
    Amf,
    /// CPU — mais lento, funciona em qualquer PC.
    SvtAv1,
}

impl Encoder {
    const PREFERENCE: [Encoder; 4] = [Encoder::Nvenc, Encoder::Qsv, Encoder::Amf, Encoder::SvtAv1];

    fn ffmpeg_name(self) -> &'static str {
        match self {
            Encoder::Nvenc => "hevc_nvenc",
            Encoder::Qsv => "hevc_qsv",
            Encoder::Amf => "hevc_amf",
            Encoder::SvtAv1 => "libsvtav1",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Encoder::Nvenc => "NVIDIA",
            Encoder::Qsv => "Intel",
            Encoder::Amf => "AMD",
            Encoder::SvtAv1 => "processador",
        }
    }

    /// Qualidade calibrada pra anime em 720p (conferido quadro a quadro).
    fn args(self) -> &'static [&'static str] {
        match self {
            Encoder::Nvenc => &["-c:v:0", "hevc_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "27", "-b:v", "0"],
            Encoder::Qsv => &["-c:v:0", "hevc_qsv", "-preset", "medium", "-global_quality", "25"],
            Encoder::Amf => &["-c:v:0", "hevc_amf", "-quality", "balanced", "-rc", "cqp", "-qp_i", "24", "-qp_p", "26"],
            Encoder::SvtAv1 => &["-c:v:0", "libsvtav1", "-preset", "8", "-crf", "34"],
        }
    }

    fn pixel_format(self) -> &'static str {
        match self {
            Encoder::SvtAv1 => "yuv420p10le",
            _ => "nv12",
        }
    }
}

static DETECTED: Mutex<Option<Encoder>> = Mutex::new(None);

/// 1º codificador que REALMENTE funciona nesse PC — estar na lista do
/// ffmpeg não basta (ex. hevc_nvenc listado mas driver velho demais, visto
/// ao vivo). Testa com 0,2s de vídeo sintético; resultado fica em cache.
pub fn detect_encoder(ffmpeg_path: &Path) -> Encoder {
    if let Some(found) = *DETECTED.lock().unwrap() {
        return found;
    }
    let found = Encoder::PREFERENCE
        .into_iter()
        .find(|enc| {
            ffmpeg::command(ffmpeg_path)
                .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i", "color=black:s=1280x720:d=0.2"])
                .args(["-c:v", enc.ffmpeg_name(), "-f", "null", "-"])
                .output()
                .is_ok_and(|o| o.status.success())
        })
        .unwrap_or(Encoder::SvtAv1);
    *DETECTED.lock().unwrap() = Some(found);
    found
}

/// Largura máxima pra cada opção (pela LARGURA, não altura: release com
/// corte cinema tipo 1920x800 continua sendo "1080p").
pub fn max_width_for(resolution: &str) -> Option<u32> {
    match resolution {
        "720p" => Some(1280),
        "480p" => Some(854),
        _ => None,
    }
}

pub fn downscale(
    paths: &ffmpeg::FfmpegPaths,
    file: &Path,
    max_width: u32,
    encoder: Encoder,
) -> Result<Outcome, ProcessError> {
    if !media_file::is_mkv(file) {
        return Ok(Outcome::Nothing);
    }
    let original = media_file::probe(&paths.ffprobe, file).map_err(ProcessError::Permanent)?;
    let Some(width) = original.main_video().and_then(|v| v.width) else {
        return Ok(Outcome::Nothing);
    };
    if width <= max_width {
        return Ok(Outcome::Nothing);
    }

    let mut cmd = ffmpeg::command(&paths.ffmpeg);
    cmd.args(["-nostdin", "-v", "error", "-y", "-i"]).arg(file);
    // Tudo copiado (áudio, legendas, anexos) — só o vídeo principal é
    // recodificado; filtro só nele (-filter:v:0), senão o ffmpeg recusa
    // capa embutida sendo copiada.
    cmd.args(["-map", "0", "-c", "copy"]);
    cmd.args(encoder.args());
    cmd.arg("-filter:v:0").arg(format!("scale={max_width}:-2:flags=lanczos,format={}", encoder.pixel_format()));
    cmd.arg(media_file::temp_path(file));
    media_file::run_and_replace(&paths.ffprobe, cmd, file, &original)
}

#[cfg(test)]
mod tests {
    use super::max_width_for;

    #[test]
    fn max_width_only_for_known_resolutions() {
        assert_eq!(max_width_for("720p"), Some(1280));
        assert_eq!(max_width_for("480p"), Some(854));
        assert_eq!(max_width_for("original"), None);
        assert_eq!(max_width_for(""), None);
    }
}
