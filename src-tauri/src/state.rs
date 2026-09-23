use crate::activity::ActivityLog;
use crate::torrent_engine::TorrentEngine;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

pub struct AppState {
    pub db: SqlitePool,
    pub http: reqwest::Client,
    pub activity: ActivityLog,
    pub poll_lock: Mutex<()>,
    pub torrent: TorrentEngine,
}
