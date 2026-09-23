//! ffmpeg/ffprobe baixados SOB DEMANDA (só quando o usuário liga uma opção
//! que precisa deles — ver `audio_strip`), em vez de irem no instalador
//! (~80MB a mais pra quem nunca usa). Build LGPL "shared" do BtbN
//! (github.com/BtbN/FFmpeg-Builds), conferida pelo SHA-256 que o próprio
//! release publica. Fica em `%LOCALAPPDATA%\com.torii.app\tools\ffmpeg\bin`.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

const RELEASE_BASE: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/";
// 8.1, não 9.0: o 9.0 exige NVENC API 13.1 (driver NVIDIA 610+) e falhou
// ao vivo numa RTX 3060 Ti com driver 595 — o 8.1 funciona com drivers
// bem mais antigos, e a redução de resolução depende do NVENC.
const ZIP_NAME: &str = "ffmpeg-n8.1-latest-win64-lgpl-shared-8.1.zip";

#[derive(Debug, Clone)]
pub struct FfmpegPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InstallStatus {
    pub installed: bool,
    pub downloading: bool,
    /// 0–100 enquanto baixa.
    pub progress: f32,
    pub error: Option<String>,
}

static STATUS: Mutex<InstallStatus> =
    Mutex::new(InstallStatus { installed: false, downloading: false, progress: 0.0, error: None });
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn tools_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("tools").join("ffmpeg"))
}

fn paths_in(dir: &Path) -> FfmpegPaths {
    let bin = dir.join("bin");
    FfmpegPaths { ffmpeg: bin.join("ffmpeg.exe"), ffprobe: bin.join("ffprobe.exe") }
}

/// Arquivo ao lado do bin com o nome do pacote instalado — trocar
/// `ZIP_NAME` (ex. 9.0 → 8.1) faz a próxima instalação substituir a antiga
/// em vez de seguir usando a versão errada.
const VERSION_FILE: &str = "installed.txt";

pub fn installed(app: &AppHandle) -> Option<FfmpegPaths> {
    let dir = tools_dir(app).ok()?;
    let version = std::fs::read_to_string(dir.join(VERSION_FILE)).ok()?;
    if version.trim() != ZIP_NAME {
        return None;
    }
    let paths = paths_in(&dir);
    (paths.ffmpeg.is_file() && paths.ffprobe.is_file()).then_some(paths)
}

pub fn status(app: &AppHandle) -> InstallStatus {
    let mut status = STATUS.lock().unwrap().clone();
    status.installed = installed(app).is_some();
    status
}

fn set_status(f: impl FnOnce(&mut InstallStatus)) {
    f(&mut STATUS.lock().unwrap());
}

/// Garante ffmpeg instalado, baixando se preciso. Chamadas concorrentes
/// esperam o mesmo download em vez de baixar 2x.
pub async fn ensure_installed(app: &AppHandle, http: &reqwest::Client) -> Result<FfmpegPaths, String> {
    let _guard = INSTALL_LOCK.lock().await;
    if let Some(paths) = installed(app) {
        return Ok(paths);
    }
    set_status(|s| {
        s.downloading = true;
        s.progress = 0.0;
        s.error = None;
    });
    let result = download_and_extract(app, http).await;
    set_status(|s| {
        s.downloading = false;
        s.error = result.as_ref().err().cloned();
    });
    result
}

async fn download_and_extract(app: &AppHandle, http: &reqwest::Client) -> Result<FfmpegPaths, String> {
    let dir = tools_dir(app)?;
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    // Versão anterior (outro ZIP_NAME) — sai inteira antes da nova entrar,
    // pra não misturar DLL de versões diferentes.
    let _ = tokio::fs::remove_dir_all(dir.join("bin")).await;
    let _ = tokio::fs::remove_file(dir.join(VERSION_FILE)).await;

    let checksums = http
        .get(format!("{RELEASE_BASE}checksums.sha256"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| tr!("não conseguiu baixar checksums do ffmpeg: {e}", "couldn't download the ffmpeg checksums: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let expected = checksums
        .lines()
        .find_map(|line| {
            let (hash, name) = line.split_once(char::is_whitespace)?;
            (name.trim().trim_start_matches('*') == ZIP_NAME).then(|| hash.trim().to_lowercase())
        })
        .ok_or_else(|| tr!("{ZIP_NAME} não aparece no checksums.sha256 do release", "{ZIP_NAME} isn't listed in the release checksums.sha256"))?;

    let zip_path = dir.join("download.zip.part");
    let mut resp = http
        .get(format!("{RELEASE_BASE}{ZIP_NAME}"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("falha ao baixar ffmpeg: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(&zip_path).await.map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut done: u64 = 0;
    while let Some(chunk) = resp.chunk().await.map_err(|e| tr!("download do ffmpeg interrompido: {e}", "ffmpeg download interrupted: {e}"))? {
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if total > 0 {
            set_status(|s| s.progress = (done as f32 / total as f32 * 100.0).min(100.0));
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        let _ = tokio::fs::remove_file(&zip_path).await;
        return Err(tr!("download do ffmpeg corrompido (SHA-256 não bate) — tente de novo", "ffmpeg download corrupted (SHA-256 mismatch) — try again"));
    }

    let extract_dir = dir.clone();
    let zip_for_extract = zip_path.clone();
    tauri::async_runtime::spawn_blocking(move || extract_bin(&zip_for_extract, &extract_dir))
        .await
        .map_err(|e| e.to_string())??;
    let _ = tokio::fs::remove_file(&zip_path).await;
    tokio::fs::write(dir.join(VERSION_FILE), ZIP_NAME).await.map_err(|e| e.to_string())?;

    installed(app).ok_or_else(|| tr!("ffmpeg.exe/ffprobe.exe não vieram no pacote", "ffmpeg.exe/ffprobe.exe missing from the package"))
}

/// Extrai só `*/bin/*` (ffmpeg, ffprobe e DLLs) — o resto do pacote
/// (headers, docs, ffplay) não é usado.
fn extract_bin(zip_path: &Path, dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let bin_dir = dir.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else { continue };
        let in_bin = name.parent().and_then(|p| p.file_name()).is_some_and(|p| p == "bin");
        let Some(file_name) = name.file_name() else { continue };
        if !in_bin || file_name == "ffplay.exe" {
            continue;
        }
        let mut out = std::fs::File::create(bin_dir.join(file_name)).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Processo do ffmpeg/ffprobe sem janela de console e com prioridade
/// abaixo do normal — roda em segundo plano sem disputar CPU com o player.
pub fn command(program: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    cmd
}
