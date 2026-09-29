//! Correção de MKV que abre devagar no player.
//!
//! O ffmpeg (Lavf) grava por padrão um CRC32 dentro de cada elemento do
//! MKV, inclusive no índice de busca (Cues). O leitor de MKV do VLC 3.0.x
//! se perde nesse CRC32 dentro do índice e varre o resto do arquivo byte a
//! byte — ~10s pra abrir qualquer release gravada com ffmpeg (visto no log
//! do libvlc). Regravar só copiando (sem recodificar) com `-write_crc32 0`
//! resolve: mesmas faixas, legendas e fontes, abre na hora.

use crate::ffmpeg::{self, FfmpegPaths};
use crate::media_file::{self, Outcome, ProcessError};
use std::io::Read;
use std::path::Path;

/// O cabeçalho (seção Info, logo no começo) diz quem gravou o arquivo — dá
/// pra saber sem ffmpeg.
pub fn written_by_ffmpeg(file: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(file) else { return false };
    let mut head = vec![0u8; 64 * 1024];
    let Ok(n) = f.read(&mut head) else { return false };
    head[..n].windows(4).any(|w| w == b"Lavf")
}

pub fn fix(paths: &FfmpegPaths, file: &Path) -> Result<Outcome, ProcessError> {
    let original = media_file::probe(&paths.ffprobe, file).map_err(ProcessError::Permanent)?;
    let mut cmd = ffmpeg::command(&paths.ffmpeg);
    cmd.args(["-nostdin", "-v", "error", "-y", "-i"]).arg(file);
    cmd.args(["-map", "0", "-c", "copy", "-write_crc32", "0"]);
    cmd.arg(media_file::temp_path(file));
    media_file::run_and_replace(&paths.ffprobe, cmd, file, &original)
}
