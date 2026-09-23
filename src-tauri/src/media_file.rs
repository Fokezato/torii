//! Base comum do pós-processamento de episódio baixado (ver `postprocess`):
//! ler as faixas do arquivo (ffprobe) e trocar o original por uma versão
//! nova com segurança.

use crate::ffmpeg;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
pub struct Probe {
    #[serde(default)]
    pub streams: Vec<ProbeStream>,
    pub format: Option<ProbeFormat>,
}

#[derive(serde::Deserialize)]
pub struct ProbeStream {
    pub index: u32,
    pub codec_type: Option<String>,
    pub width: Option<u32>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub disposition: HashMap<String, i64>,
}

#[derive(serde::Deserialize)]
pub struct ProbeFormat {
    pub duration: Option<String>,
}

impl Probe {
    pub fn duration_secs(&self) -> Option<f64> {
        self.format.as_ref()?.duration.as_deref()?.parse().ok()
    }

    /// Vídeo principal — ignora capa/miniatura embutida (attached_pic).
    pub fn main_video(&self) -> Option<&ProbeStream> {
        self.streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.get("attached_pic") != Some(&1))
    }
}

pub fn probe(ffprobe: &Path, file: &Path) -> Result<Probe, String> {
    let out = ffmpeg::command(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=index,codec_type,width:stream_tags=language:stream_disposition=default,attached_pic",
        ])
        .args(["-show_entries", "format=duration", "-of", "json"])
        .arg(file)
        .output()
        .map_err(|e| format!("ffprobe não rodou: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffprobe falhou: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("saída do ffprobe inválida: {e}"))
}

/// Resultado de uma operação num arquivo.
pub enum Outcome {
    /// Nada a fazer nesse arquivo.
    Nothing,
    /// Arquivo trocado; `saved` bytes a menos.
    Replaced { saved: u64 },
}

/// `Retry`: passageiro (arquivo em uso) — tenta de novo no próximo ciclo.
/// `Permanent`: esse arquivo não dá — marca como processado e segue.
pub enum ProcessError {
    Retry,
    Permanent(String),
}

/// Caminho temporário ao lado do original ("Ep.mkv" → "Ep.torii-tmp.mkv").
pub fn temp_path(file: &Path) -> PathBuf {
    file.with_extension("torii-tmp.mkv")
}

/// Roda o ffmpeg já montado (saída = `temp_path(file)`), confere o
/// resultado e troca pelo original: duração tem que bater (±2s). Original
/// vira `.torii-old` antes, e só é apagado depois do novo assumir o nome —
/// em nenhum momento fica sem arquivo válido.
pub fn run_and_replace(
    ffprobe: &Path,
    mut cmd: std::process::Command,
    file: &Path,
    original: &Probe,
) -> Result<Outcome, ProcessError> {
    let tmp = temp_path(file);
    let out = cmd.output().map_err(|e| ProcessError::Permanent(format!("ffmpeg não rodou: {e}")))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&tmp);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        return Err(ProcessError::Permanent(format!("ffmpeg falhou: {last}")));
    }

    let ok = match (probe(ffprobe, &tmp), original.duration_secs()) {
        (Ok(p), Some(orig)) => p.duration_secs().is_some_and(|d| (d - orig).abs() <= 2.0),
        _ => false,
    };
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(ProcessError::Permanent("arquivo gerado não confere com o original".to_string()));
    }

    let before = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    let after = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);

    // Falha no rename é quase sempre arquivo em uso (aberto em outro player).
    let old = file.with_extension("torii-old.mkv");
    if std::fs::rename(file, &old).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(ProcessError::Retry);
    }
    if std::fs::rename(&tmp, file).is_err() {
        let _ = std::fs::rename(&old, file);
        let _ = std::fs::remove_file(&tmp);
        return Err(ProcessError::Retry);
    }
    let _ = std::fs::remove_file(&old);
    Ok(Outcome::Replaced { saved: before.saturating_sub(after) })
}

pub fn is_mkv(file: &Path) -> bool {
    file.extension().is_some_and(|e| e.eq_ignore_ascii_case("mkv"))
}
