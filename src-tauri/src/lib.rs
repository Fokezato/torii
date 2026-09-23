#[macro_use]
mod i18n;
mod activity;
mod audio_strip;
mod clean_filename;
mod commands;
mod db;
mod downscale;
mod engine;
mod error;
mod ffmpeg;
mod media_file;
mod postprocess;
mod jellyfin;
mod notify;
mod player;
mod sources;
mod state;
mod torrent_engine;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

const NOTIFICATION_WINDOW_SIZE: (f64, f64) = (360.0, 96.0);
const NOTIFICATION_WINDOW_MARGIN: (i32, i32) = (16, 60);

/// Janela flutuante sem borda pro toast de notificação (ver
/// `src/routes/NotificationWindow`). Não é toast nativo do Windows — aquele
/// exige AUMID registrado e continuou sendo descartado mesmo no app
/// instalado de verdade. Criada escondida, a própria página mostra/esconde
/// ela via `getCurrentWindow().show()/hide()` conforme a fila de eventos
/// `notify:show`. Fica sempre no topo e some da taskbar, então aparece
/// mesmo com a janela principal minimizada na tray.
fn spawn_notification_window(app: &tauri::App) -> tauri::Result<()> {
    let (width, height) = NOTIFICATION_WINDOW_SIZE;
    let window = WebviewWindowBuilder::new(
        app,
        "notification",
        WebviewUrl::App("index.html#/notification-window".into()),
    )
    .title("")
    .inner_size(width, height)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    // Criada já visível de propósito: um window criado com visible=false e
    // "mostrado" depois via show() nunca aparece nesse ambiente — mesmo bug
    // que já pegou a janela principal (ver tray click handler). Fica sempre
    // mapeada; o conteúdo em branco/transparente já basta pra "esconder".
    .visible(true)
    .focused(false)
    .build()?;

    // Sempre mapeada + always-on-top no canto inferior direito, ela engolia
    // clique de qualquer coisa embaixo — com o player grande (janela
    // maximizada num ultrawide), os botões de baixo à direita dos controles
    // caíam exatamente sob ela e paravam de responder (bug real reportado).
    // O toast não tem nada clicável, então deixa o clique passar sempre.
    let _ = window.set_ignore_cursor_events(true);

    if let Ok(Some(monitor)) = window.primary_monitor() {
        let screen = monitor.size();
        let (margin_x, margin_y) = NOTIFICATION_WINDOW_MARGIN;
        let scale = monitor.scale_factor();
        let win_w = (width * scale) as i32;
        let win_h = (height * scale) as i32;
        let x = screen.width as i32 - win_w - (margin_x as f64 * scale) as i32;
        let y = screen.height as i32 - win_h - (margin_y as f64 * scale) as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }

    Ok(())
}

/// Janela transparente dos controles do player (ver `routes/PlayerOverlay`)
/// — criada 1x no boot, igual a de notificação, em vez de sob demanda ao
/// entrar na página do player. Criar/fechar por página tinha uma race real
/// com o StrictMode do React em dev (efeito roda 2x: cria, desmonta/fecha,
/// remonta/cria de novo — as chamadas open/close concorrentes bagunçavam a
/// relação de owner window e deixavam a janela principal inteira sem
/// responder a clique). Existindo a vida toda, a página do player só
/// reposiciona ela (`player_set_video_area`), nunca cria/destrói.
fn spawn_player_overlay_window(app: &tauri::App, main: &tauri::WebviewWindow) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "player-overlay", WebviewUrl::App("index.html#/player-overlay".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .visible(true)
        .focused(false)
        .inner_size(1.0, 1.0)
        .position(-2000.0, -2000.0)
        .owner(main)?
        .build()?;
    Ok(())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Tem que ser o primeiro plugin registrado (recomendação da própria
        // doc do Tauri) pra funcionar direito no Windows. Segunda instância
        // não abre outra janela nem duplica engine/torrent/tray — só foca a
        // que já tá rodando.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Spawned em vez de chamado direto: já vimos nessa mesma base de
            // código (tray click handler) que chamar show()/set_focus() de
            // forma síncrona reentrante no main thread é pouco confiável.
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.maximize();
                    let _ = window.set_focus();
                }
            });
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let db_path = app.path().app_data_dir()?.join("torii.db");
            let default_library_root = app
                .path()
                .video_dir()
                .map(|dir| dir.join("Torii").join("Series"))
                .unwrap_or_else(|_| db_path.parent().unwrap().join("Series"));

            let downloads_root = default_library_root.clone();
            let (pool, torrent) = tauri::async_runtime::block_on(async {
                let pool = db::init_pool(&db_path).await.expect("failed to init db");
                db::settings::seed_defaults(&pool, &default_library_root.to_string_lossy())
                    .await
                    .expect("failed to seed default settings");
                if let Ok(settings) = db::settings::get_all(&pool).await {
                    i18n::apply_setting(settings.get("app_language").map(String::as_str).unwrap_or("auto"));
                }
                let torrent = torrent_engine::TorrentEngine::new(downloads_root)
                    .await
                    .expect("failed to start torrent engine");
                (pool, torrent)
            });

            app.manage(state::AppState {
                db: pool,
                http: reqwest::Client::new(),
                activity: activity::ActivityLog::new(),
                poll_lock: tokio::sync::Mutex::new(()),
                torrent,
            });
            app.manage(player::PlayerState::default());

            tauri::async_runtime::spawn(engine::backfill_placeholder_episodes(app.handle().clone()));
            tauri::async_runtime::spawn(engine::backfill_series(app.handle().clone()));
            engine::spawn_background_loop(app.handle().clone());
            engine::spawn_download_reconciler(app.handle().clone());
            tauri::async_runtime::spawn(engine::resume_pending_downloads(app.handle().clone()));
            tauri::async_runtime::spawn(engine::resync_jellyfin_library(app.handle().clone()));

            let open_i = MenuItem::with_id(app, "open", tr!("Abrir", "Open"), true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", tr!("Sair", "Quit"), true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_i, &quit_i])?;
            app.manage(i18n::TrayMenu { open: open_i.clone(), quit: quit_i.clone() });

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Torii")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle().clone();
                        // Dispatched off the main thread on purpose: this callback runs
                        // reentrant on the main thread (nested inside the tray WndProc),
                        // and Tauri's window ops special-case "already on main thread" by
                        // running inline instead of queuing through the event loop, which
                        // is unreliable from here. Spawning takes the normal queued path.
                        tauri::async_runtime::spawn(async move {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.maximize();
                                let _ = window.set_focus();
                            }
                        });
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.maximize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                // Autostart passa "--hidden" (ver .plugin(tauri_plugin_autostart::init))
                // pra abrir direto na bandeja em vez de mostrar a janela.
                if std::env::args().any(|a| a == "--hidden") {
                    let _ = window.hide();
                }

                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        // Sempre previne o close nativo primeiro (síncrono,
                        // tem que ser aqui); a settings (tray ou sair de vez)
                        // só dá pra ler de forma assíncrona.
                        api.prevent_close();
                        let app = window_clone.app_handle().clone();
                        let window_clone = window_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<state::AppState>();
                            let close_action = db::settings::get_all(&state.db)
                                .await
                                .ok()
                                .and_then(|s| s.get("close_action").cloned())
                                .unwrap_or_else(|| "tray".to_string());
                            if close_action == "quit" {
                                app.exit(0);
                            } else {
                                let _ = window_clone.hide();
                                // Overlay do player é janela própria — sem
                                // isso ficava flutuando sobre a área de
                                // trabalho com o app na bandeja (bug real).
                                if let Some(overlay) = app.get_webview_window("player-overlay") {
                                    let _ = overlay.set_position(PhysicalPosition::new(-32000, -32000));
                                }
                            }
                        });
                    }

                    // A doc do Win32 diz que janela "owned" some sozinha
                    // quando a dona minimiza, mas isso não se confirmou na
                    // prática aqui (overlay do player ficava flutuando por
                    // cima da área de trabalho com o app minimizado — bug
                    // real reportado) — força na mão. Dispatched pra não
                    // ser a chamada reentrante de `is_minimized()` no main
                    // thread (mesmo motivo dos outros spawns aqui).
                    if let WindowEvent::Resized(_) = event {
                        let app = window_clone.app_handle().clone();
                        let window_clone = window_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            if window_clone.is_minimized().unwrap_or(false) {
                                if let Some(overlay) = app.get_webview_window("player-overlay") {
                                    let _ = overlay.set_position(PhysicalPosition::new(-32000, -32000));
                                }
                            }
                        });
                    }
                });

                // Child HWND nativa pro vídeo do libvlc + carrega o motor.
                // Best-effort de propósito: se a DLL vendorizada faltar ou
                // o load falhar, loga e segue sem player em vez de derrubar
                // o app inteiro (ver `player::PlayerState`).
                #[cfg(windows)]
                if let Ok(main_hwnd) = window.hwnd() {
                    match player::media_session::MediaSession::new(app.handle(), main_hwnd) {
                        Ok(session) => {
                            *app.state::<player::PlayerState>().media_session.lock().unwrap() = Some(session);
                        }
                        Err(e) => eprintln!("[player] controle de mídia do Windows indisponível: {e}"),
                    }
                }

                #[cfg(windows)]
                match window.hwnd() {
                    Ok(main_hwnd) => match player::window::create_child(main_hwnd) {
                        Ok(child_hwnd) => match player::PlayerEngine::new(child_hwnd) {
                            Ok(engine) => {
                                *app.state::<player::PlayerState>().engine.lock().unwrap() = Some(engine);
                            }
                            Err(e) => eprintln!("[player] falha ao iniciar libvlc: {e}"),
                        },
                        Err(e) => eprintln!("[player] falha ao criar child window: {e:?}"),
                    },
                    Err(e) => eprintln!("[player] falha ao obter hwnd da janela principal: {e}"),
                }

                if let Err(e) = spawn_player_overlay_window(app, &window) {
                    eprintln!("[player] falha ao criar janela de overlay: {e}");
                }
            }

            spawn_notification_window(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::season::browse_season,
            commands::season::browse_trending,
            commands::season::anilist_search,
            commands::season::anime_seasons,
            commands::season::get_anime_news,
            commands::season::get_schedule,
            commands::watches::list_watches,
            commands::watches::create_watch,
            commands::watches::delete_watch,
            commands::watches::set_watch_active,
            commands::watches::set_watch_rating,
            commands::watches::set_watch_list_status,
            commands::watches::set_watch_preferences,
            commands::stats::get_storage_stats,
            commands::nyaa::nyaa_search,
            commands::nyaa::nyaa_available_languages,
            commands::activity::get_activity_log,
            commands::episodes::list_recent_episodes,
            commands::episodes::list_available_episodes,
            commands::episodes::list_watch_episodes,
            commands::episodes::pause_episode_download,
            commands::episodes::resume_episode_download,
            commands::episodes::cancel_episode_download,
            commands::episodes::list_episode_sources,
            commands::episodes::switch_episode_source,
            commands::episodes::force_check_episode,
            commands::player::player_open,
            commands::player::player_play,
            commands::player::player_set_paused,
            commands::player::player_stop,
            commands::player::player_seek,
            commands::player::player_seek_relative,
            commands::player::player_set_volume,
            commands::player::player_list_audio_tracks,
            commands::player::player_set_audio_track,
            commands::player::player_list_subtitle_tracks,
            commands::player::player_set_subtitle_track,
            commands::player::player_snapshot,
            commands::player::player_resize,
            commands::player::player_bring_to_front,
            commands::player::player_set_visible,
            commands::player::player_set_video_area,
            commands::player::player_get_skip_segments,
            commands::player::player_save_progress,
            commands::tools::ffmpeg_status,
            commands::tools::ffmpeg_install,
            commands::tools::translate_text,
            commands::player::media_probe,
            commands::player::media_frame,
            commands::jellyfin::test_jellyfin_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
