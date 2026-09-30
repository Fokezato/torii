use crate::ffmpeg::{self, FfmpegPaths};
use crate::media_file::{self, Outcome, ProcessError};
use std::io::Read;
use std::path::Path;

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
