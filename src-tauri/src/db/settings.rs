use sqlx::SqlitePool;
use std::collections::HashMap;

const DEFAULTS: &[(&str, &str)] = &[
    ("jellyfin_mode", "0"),
    ("jellyfin_url", "http://localhost:8096"),
    ("jellyfin_api_key", ""),
    ("poll_interval_minutes", "30"),
    ("default_delete_after_days", ""),
    ("paused", "0"),
    // "native" (player do Torii) ou "external" (player padrão do Windows).
    ("player_mode", "native"),
    // Idioma preferido do player nativo (áudio/legenda, separados por
    // vírgula, mesmo formato de watches.audio_lang/sub_lang) — filtra o
    // botão de faixas pra não listar as 10+ que um repack costuma
    // embutir, e auto-seleciona a preferida ao abrir um episódio.
    ("player_preferred_audio_langs", ""),
    ("player_preferred_subtitle_langs", ""),
    // Pular trechos (ver src/sources/aniskip.rs) — segmentos vêm da AniSkip.
    // Pulo automático e "próximo episódio depois do encerramento" vêm
    // desligados (só mudam o comportamento se o usuário ligar); marcação na
    // barra vem ligada (só visual), uma chave por tipo de trecho.
    ("player_auto_skip_intro", "0"),
    ("player_auto_skip_ending", "0"),
    ("player_auto_skip_recap", "0"),
    ("player_next_after_ending", "0"),
    ("player_mark_intro", "1"),
    ("player_mark_ending", "1"),
    ("player_mark_recap", "1"),
    // Detecta abertura/encerramento comparando o áudio dos episódios quando
    // o AniSkip não tem os tempos (ver src/intro_detect.rs). Usa o ffmpeg.
    ("detect_segments", "1"),
    // Apagar episódio depois de assistido (player nativo marca "assistido"
    // ao chegar no encerramento/90%) — desligado por padrão; espera N horas
    // antes de apagar, pra dar tempo de rever.
    ("delete_after_watched", "0"),
    ("delete_after_watched_hours", "24"),
    // Beta: tira do arquivo as faixas de áudio fora dos idiomas preferidos
    // (ver src/audio_strip.rs). Baixa o ffmpeg na 1ª vez que é ligado.
    ("strip_unused_audio", "0"),
    // Beta: recodifica o vídeo pra resolução menor (ver src/downscale.rs).
    // "original" = desligado; "720p" = ligado pra TODOS os animes (cada
    // anime só decide sozinho quando isso está desligado).
    ("downscale_resolution", "original"),
    ("close_action", "tray"),
    ("app_language", "auto"),
    // Notificações: chave mestra desliga tudo de uma vez; cada tipo tem a
    // própria chave pra desligar individualmente. Ver src/lib/notify.ts.
    ("notify_master", "1"),
    ("notify_sound", "1"),
    ("notify_found", "1"),
    ("notify_calendar", "1"),
    ("notify_error", "1"),
    ("notify_complete", "1"),
    ("notify_jellyfin", "1"),
    ("notify_watch_reminder", "1"),
    ("notify_delete_reminder", "1"),
];

pub async fn seed_defaults(pool: &SqlitePool, default_library_root: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (key, value) in DEFAULTS {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)")
            .bind(*key)
            .bind(*value)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('library_root', ?)")
        .bind(default_library_root)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn get_all(pool: &SqlitePool) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT key, value FROM settings").fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(k, v)| (k, v.unwrap_or_default()))
        .collect())
}

pub async fn update(pool: &SqlitePool, values: &HashMap<String, String>) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (key, value) in values {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
