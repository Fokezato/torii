//! Fila de pós-processamento de episódio baixado (beta, opt-in):
//! 1. "Remover áudios extras" (`audio_strip`) — rápido, sem perda;
//! 2. "Reduzir resolução" (`downscale`) — recodifica, minutos por episódio.
//!
//! Roda em segundo plano, 1 episódio por vez, nunca 2 rodadas em paralelo,
//! sem travar a busca de episódios. Nunca mexe no episódio aberto no player.
//! Cada etapa marca o episódio como feito — não reprocessa.

use crate::media_file::{Outcome, ProcessError};
use crate::{audio_strip, db, downscale, ffmpeg, state::AppState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn spawn_pending(app: &AppHandle, playing_path: Option<String>) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        process_pending(&app, playing_path).await;
        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn label_of(ep: &db::episodes::Episode) -> String {
    ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id))
}

async fn run_blocking<F>(f: F) -> Result<Outcome, ProcessError>
where
    F: FnOnce() -> Result<Outcome, ProcessError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| Err(ProcessError::Permanent(e.to_string())))
}

async fn process_pending(app: &AppHandle, playing_path: Option<String>) {
    let state = app.state::<AppState>();
    let Ok(settings) = db::settings::get_all(&state.db).await else { return };

    // Regra: ligado na Config = vale pra TODOS os animes; desligado na
    // Config = cada anime decide (opção dele, escolhida ao adicionar ou nas
    // preferências).
    let global_strip = settings.get("strip_unused_audio").map(String::as_str) == Some("1");
    let preferred = audio_strip::parse_preferred(
        settings.get("player_preferred_audio_langs").map(String::as_str).unwrap_or(""),
    );
    let global_width = downscale::max_width_for(settings.get("downscale_resolution").map(String::as_str).unwrap_or(""));

    let Ok(watches) = db::watches::list(&state.db).await else { return };
    // Sem idioma preferido não tem critério pra remover áudio — pula a etapa.
    let strip_for: HashSet<i64> = watches
        .iter()
        .filter(|w| !preferred.is_empty() && (global_strip || w.strip_audio))
        .map(|w| w.id)
        .collect();
    let max_width: HashMap<i64, u32> = watches
        .iter()
        .filter_map(|w| global_width.or_else(|| downscale::max_width_for(w.max_resolution.as_deref()?)).map(|mw| (w.id, mw)))
        .collect();
    if strip_for.is_empty() && max_width.is_empty() {
        return;
    }

    let Ok(episodes) = db::episodes::list_postprocess_pending(&state.db).await else { return };
    let mut paths: Option<ffmpeg::FfmpegPaths> = None;

    for ep in episodes {
        let Some(path) = ep.item_path.clone() else { continue };
        if playing_path.as_deref() == Some(path.as_str()) {
            continue;
        }
        let need_audio = strip_for.contains(&ep.watch_id) && ep.audio_processed_at.is_none();
        let target_width = max_width.get(&ep.watch_id).copied().filter(|_| ep.video_processed_at.is_none());
        if !need_audio && target_width.is_none() {
            continue;
        }

        // ffmpeg só é baixado quando de fato tem episódio pra processar.
        if paths.is_none() {
            match ffmpeg::ensure_installed(app, &state.http).await {
                Ok(p) => paths = Some(p),
                Err(e) => {
                    state.activity.error(tr!("Pós-processamento: {e}", "Post-processing: {e}"));
                    return;
                }
            }
        }
        let paths = paths.clone().unwrap();

        // Arquivo trocado = hash do torrent não bate mais; sai do motor
        // antes pra ele não "consertar" (rebaixar) o arquivo.
        let _ = state.torrent.remove(ep.id, false).await;
        let label = label_of(&ep);

        if need_audio {
            let (p, file, pref) = (paths.clone(), PathBuf::from(&path), preferred.clone());
            match run_blocking(move || audio_strip::strip(&p, &file, &pref)).await {
                Ok(outcome) => {
                    let _ = db::episodes::mark_audio_processed(&state.db, ep.id).await;
                    if let Outcome::Replaced { saved } = outcome {
                        state.activity.info(tr!("Áudios extras removidos (-{} MB): {label}", "Extra audio removed (-{} MB): {label}", saved / 1_000_000));
                    }
                }
                Err(ProcessError::Retry) => continue,
                Err(ProcessError::Permanent(e)) => {
                    let _ = db::episodes::mark_audio_processed(&state.db, ep.id).await;
                    state.activity.error(tr!("Remover áudios extras falhou em \"{label}\": {e}", "Removing extra audio failed on \"{label}\": {e}"));
                }
            }
        }

        if let Some(width) = target_width {
            let (p, file) = (paths.clone(), PathBuf::from(&path));
            let encoder = {
                let ffmpeg_path = paths.ffmpeg.clone();
                tauri::async_runtime::spawn_blocking(move || downscale::detect_encoder(&ffmpeg_path))
                    .await
                    .unwrap_or(downscale::Encoder::SvtAv1)
            };
            state.activity.info(tr!("Reduzindo resolução ({}): {label}", "Downscaling ({}): {label}", encoder.label()));
            match run_blocking(move || downscale::downscale(&p, &file, width, encoder)).await {
                Ok(outcome) => {
                    let _ = db::episodes::mark_video_processed(&state.db, ep.id).await;
                    if let Outcome::Replaced { saved } = outcome {
                        state.activity.info(tr!("Resolução reduzida (-{} MB): {label}", "Downscaled (-{} MB): {label}", saved / 1_000_000));
                    }
                }
                Err(ProcessError::Retry) => {}
                Err(ProcessError::Permanent(e)) => {
                    let _ = db::episodes::mark_video_processed(&state.db, ep.id).await;
                    state.activity.error(tr!("Reduzir resolução falhou em \"{label}\": {e}", "Downscaling failed on \"{label}\": {e}"));
                }
            }
        }
    }
}
