#[macro_use]
mod i18n;
mod activity;
mod audio_strip;
mod clean_filename;
mod commands;
mod db;
mod discord;
mod downscale;
mod engine;
mod error;
mod ffmpeg;
mod intro_detect;
mod media_file;
mod mkv_fix;
mod subtitles;
mod postprocess;
mod reminders;
mod jellyfin;
mod notify;
mod player;
mod sources;
mod state;
mod stream_server;
mod torrent_engine;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

const NOTIFICATION_WINDOW_SIZE: (f64, f64) = (360.0, 96.0);
const NOTIFICATION_WINDOW_MARGIN: (i32, i32) = (16, 60);

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
    .visible(true)
    .focused(false)
    .build()?;

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

fn spawn_player_overlay_window(app: &tauri::App, main: &tauri::WebviewWindow) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, "player-overlay", WebviewUrl::App("index.html#/player-overlay".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .visible(cfg!(windows))
        .focused(false)
        .inner_size(1.0, 1.0)
        .position(-2000.0, -2000.0);
    #[cfg(windows)]
    let builder = builder.owner(main)?;
    #[cfg(not(windows))]
    let builder = builder.parent(main)?;
    builder.build()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // libVLC can only embed into X11 windows; run under XWayland on Wayland.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            app.manage(discord::Presence::start());
            match tauri::async_runtime::block_on(stream_server::start(app.handle().clone())) {
                Ok(server) => {
                    app.manage(server);
                }
                Err(e) => eprintln!("[stream] failed to start: {e}"),
            }

            tauri::async_runtime::spawn(engine::backfill_placeholder_episodes(app.handle().clone()));
            tauri::async_runtime::spawn(engine::backfill_series(app.handle().clone()));
            engine::spawn_watched_cleanup_loop(app.handle().clone());
            engine::spawn_background_loop(app.handle().clone());
            engine::spawn_download_reconciler(app.handle().clone());
            tauri::async_runtime::spawn(engine::resume_pending_downloads(app.handle().clone()));
            {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    engine::repair_missing_item_paths(&app.state::<state::AppState>()).await;
                });
            }
            tauri::async_runtime::spawn(engine::resync_jellyfin_library(app.handle().clone()));
            {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    intro_detect::spawn_pending(&app);
                });
            }

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
                if std::env::args().any(|a| a == "--hidden") {
                    let _ = window.hide();
                }

                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
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
                                let main_app = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    let state = main_app.state::<player::PlayerState>();
                                    let _ = commands::player::player_stop(main_app.clone(), state.clone());
                                    let _ = commands::player::player_set_visible(state, false);
                                });
                                let _ = app.emit_to("main", "app:window-hidden", ());
                                let _ = window_clone.hide();
                                if let Some(overlay) = app.get_webview_window("player-overlay") {
                                    commands::player::hide_overlay(&overlay);
                                }
                            }
                        });
                    }

                    if let WindowEvent::Resized(_) = event {
                        let app = window_clone.app_handle().clone();
                        let window_clone = window_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            if window_clone.is_minimized().unwrap_or(false) {
                                if let Some(overlay) = app.get_webview_window("player-overlay") {
                                    commands::player::hide_overlay(&overlay);
                                }
                            }
                        });
                    }
                });

                #[cfg(target_os = "linux")]
                match player::media_session::MediaSession::new(app.handle()) {
                    Ok(session) => {
                        *app.state::<player::PlayerState>().media_session.lock().unwrap() = Some(session);
                    }
                    Err(e) => eprintln!("[player] MPRIS media controls unavailable: {e}"),
                }
                #[cfg(windows)]
                if let Ok(main_hwnd) = window.hwnd() {
                    match player::media_session::MediaSession::new(app.handle(), main_hwnd) {
                        Ok(session) => {
                            *app.state::<player::PlayerState>().media_session.lock().unwrap() = Some(session);
                        }
                        Err(e) => eprintln!("[player] Windows media controls unavailable: {e}"),
                    }
                }

                match player::window::main_surface(&window) {
                    Some(main_surface) => match player::window::create_child(main_surface) {
                        Ok(child) => match player::PlayerEngine::new(child) {
                            Ok(engine) => {
                                let state = app.state::<player::PlayerState>();
                                state
                                    .video_hwnd
                                    .store(player::window::to_raw(child), std::sync::atomic::Ordering::Relaxed);
                                *state.engine.lock().unwrap() = Some(engine);
                            }
                            Err(e) => eprintln!("[player] failed to start libvlc: {e}"),
                        },
                        Err(e) => eprintln!("[player] failed to create the video window: {e}"),
                    },
                    None => eprintln!("[player] main window has no native handle (pure Wayland?)"),
                }

                if let Err(e) = spawn_player_overlay_window(app, &window) {
                    eprintln!("[player] failed to create the overlay window: {e}");
                }
            }

            spawn_notification_window(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::season::browse_season,
            commands::season::browse_trending,
            commands::season::anilist_search,
            commands::season::anilist_catalog,
            commands::season::anime_seasons,
            commands::season::get_anime_news,
            commands::season::get_schedule,
            commands::watches::list_watches,
            commands::watches::create_watch,
            commands::watches::remove_watch,
            commands::watches::set_watch_active,
            commands::watches::set_watch_rating,
            commands::watches::set_watch_list_status,
            commands::watches::set_watch_preferences,
            commands::stats::get_storage_stats,
            commands::nyaa::nyaa_available_languages,
            commands::activity::get_activity_log,
            commands::episodes::list_recent_episodes,
            commands::episodes::list_available_episodes,
            commands::episodes::list_watch_episodes,
            commands::episodes::pause_episode_download,
            commands::episodes::resume_episode_download,
            commands::episodes::cancel_episode_download,
            commands::episodes::delete_episode,
            commands::episodes::list_episode_sources,
            commands::episodes::download_missing_episodes,
            commands::episodes::episode_stream_url,
            commands::episodes::episode_stream_start,
            commands::episodes::episode_prefetch_next,
            commands::episodes::switch_episode_source,
            commands::episodes::force_check_episode,
            commands::player::player_open,
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
            commands::player::player_bring_to_front,
            commands::player::player_set_visible,
            commands::player::player_set_video_area,
            commands::player::player_get_skip_segments,
            commands::player::player_save_progress,
            commands::player::player_ambient_frame,
            commands::player::player_subtitle_cues,
            commands::tools::ffmpeg_status,
            commands::tools::ffmpeg_install,
            commands::tools::translate_text,
            commands::player::media_probe,
            commands::player::media_frame,
            commands::jellyfin::test_jellyfin_connection,
            commands::jellyfin::jellyfin_users,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
