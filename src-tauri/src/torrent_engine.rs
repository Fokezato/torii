use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, TorrentStats};
use std::collections::{HashMap, HashSet};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const ADD_TORRENT_TIMEOUT: Duration = Duration::from_secs(45);
const INIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Episodes map to torrents; several episodes can share one season-pack torrent, each
/// pointing at its own file (`files`). Only the files of tracked episodes are downloaded.
pub struct TorrentEngine {
    session: Arc<Session>,
    active: Mutex<HashMap<i64, Arc<ManagedTorrent>>>,
    files: Mutex<HashMap<i64, usize>>,
}

#[derive(Debug, Clone)]
pub struct PackFile {
    pub index: usize,
    pub path: String,
    pub len: u64,
}

fn timeout_error() -> anyhow::Error {
    anyhow::anyhow!(tr!(
        "timeout resolvendo metadata do torrent (sem peers respondendo)",
        "timed out resolving torrent metadata (no peers responding)"
    ))
}

impl TorrentEngine {
    pub async fn new(default_output_folder: PathBuf) -> anyhow::Result<Self> {
        let session = Session::new(default_output_folder).await?;
        Ok(Self { session, active: Mutex::new(HashMap::new()), files: Mutex::new(HashMap::new()) })
    }

    /// `file_index`: the episode's file inside a season pack; `None` = single-episode torrent.
    pub async fn add_download(
        &self,
        episode_id: i64,
        magnet: &str,
        output_folder: &str,
        file_index: Option<usize>,
    ) -> anyhow::Result<String> {
        let opts = AddTorrentOptions {
            output_folder: Some(output_folder.to_string()),
            overwrite: true,
            only_files: file_index.map(|i| vec![i]),
            ..Default::default()
        };
        let response =
            tokio::time::timeout(ADD_TORRENT_TIMEOUT, self.session.add_torrent(AddTorrent::from_url(magnet), Some(opts)))
                .await
                .map_err(|_| timeout_error())??;
        let (handle, already_managed) = match response {
            AddTorrentResponse::Added(_, handle) => (handle, false),
            AddTorrentResponse::AlreadyManaged(_, handle) => (handle, true),
            AddTorrentResponse::ListOnly(_) => anyhow::bail!(tr!(
                "torrent ficou list-only, sem handle pra rastrear",
                "torrent ended up list-only, no handle to track"
            )),
        };
        let info_hash = handle.info_hash().as_string();

        let mut active = self.active.lock().await;
        let mut files = self.files.lock().await;
        active.insert(episode_id, handle.clone());
        match file_index {
            Some(index) => {
                files.insert(episode_id, index);
                if already_managed {
                    let wanted = shared_files(&active, &files, &handle);
                    drop((active, files));
                    self.update_files(&handle, &wanted).await?;
                }
            }
            None => {
                files.remove(&episode_id);
            }
        }
        Ok(info_hash)
    }

    /// Files of a torrent without downloading it.
    pub async fn list_files(&self, magnet: &str) -> anyhow::Result<Vec<PackFile>> {
        let opts = AddTorrentOptions { list_only: true, ..Default::default() };
        let response =
            tokio::time::timeout(ADD_TORRENT_TIMEOUT, self.session.add_torrent(AddTorrent::from_url(magnet), Some(opts)))
                .await
                .map_err(|_| timeout_error())??;
        let files = match response {
            AddTorrentResponse::ListOnly(list) => list
                .info
                .iter_file_details()
                .enumerate()
                .filter(|(_, f)| !f.attrs().padding)
                .map(|(index, f)| PackFile { index, path: f.filename.to_pathbuf().to_string_lossy().to_string(), len: f.len })
                .collect(),
            AddTorrentResponse::AlreadyManaged(_, handle) | AddTorrentResponse::Added(_, handle) => {
                let meta = handle.metadata.load();
                meta.as_ref()
                    .map(|m| {
                        m.file_infos
                            .iter()
                            .enumerate()
                            .map(|(index, f)| PackFile {
                                index,
                                path: f.relative_filename.to_string_lossy().to_string(),
                                len: f.len,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
        };
        Ok(files)
    }

    /// librqbit refuses to change the file selection while a torrent is still checking
    /// its files, which happens right after a pack is added.
    async fn update_files(&self, handle: &Arc<ManagedTorrent>, wanted: &HashSet<usize>) -> anyhow::Result<()> {
        let _ = tokio::time::timeout(INIT_TIMEOUT, handle.wait_until_initialized()).await;
        self.session.update_only_files(handle, wanted).await
    }

    pub async fn pause(&self, episode_id: i64) -> anyhow::Result<()> {
        let handle = self.active.lock().await.get(&episode_id).cloned();
        if let Some(handle) = handle {
            self.session.pause(&handle).await?;
        }
        Ok(())
    }

    pub async fn resume(&self, episode_id: i64) -> anyhow::Result<()> {
        let handle = self.active.lock().await.get(&episode_id).cloned();
        if let Some(handle) = handle {
            self.session.unpause(&handle).await?;
        }
        Ok(())
    }

    /// Per-episode stats; for season packs, progress and completion of the episode's file.
    pub async fn snapshot(&self) -> Vec<(i64, TorrentStats)> {
        let active = self.active.lock().await;
        let files = self.files.lock().await;
        active
            .iter()
            .map(|(episode_id, handle)| {
                let mut stats = handle.stats();
                if let Some(&index) = files.get(episode_id) {
                    let len = handle
                        .metadata
                        .load()
                        .as_ref()
                        .and_then(|m| m.file_infos.get(index).map(|f| f.len))
                        .unwrap_or(0);
                    let done = stats.file_progress.get(index).copied().unwrap_or(0);
                    stats.progress_bytes = done;
                    stats.total_bytes = len;
                    stats.finished = len > 0 && done >= len;
                }
                (*episode_id, stats)
            })
            .collect()
    }

    async fn episode_file(&self, episode_id: i64) -> Option<(Arc<ManagedTorrent>, usize)> {
        let handle = self.active.lock().await.get(&episode_id).cloned()?;
        let index = match self.files.lock().await.get(&episode_id).copied() {
            Some(index) => index,
            None => {
                let meta = handle.metadata.load();
                meta.as_ref()?.file_infos.iter().enumerate().max_by_key(|(_, f)| f.len)?.0
            }
        };
        Some((handle, index))
    }

    pub async fn stream_target(&self, episode_id: i64) -> Option<(Arc<ManagedTorrent>, usize, u64, String)> {
        let (handle, index) = self.episode_file(episode_id).await?;
        let meta = handle.metadata.load();
        let file = meta.as_ref()?.file_infos.get(index)?;
        let (name, len) = (file.relative_filename.to_string_lossy().to_string(), file.len);
        drop(meta);
        Some((handle, index, len, name))
    }

    pub async fn primary_file_path(&self, episode_id: i64) -> Option<PathBuf> {
        let (handle, index) = self.episode_file(episode_id).await?;
        let meta = handle.metadata.load();
        let file = meta.as_ref()?.file_infos.get(index)?;
        Some(handle.output_folder().join(&file.relative_filename))
    }

    /// Stops tracking the episode. A pack shared with other episodes keeps running
    /// without this episode's file (deleted from disk when `delete_files`).
    pub async fn remove(&self, episode_id: i64, delete_files: bool) -> anyhow::Result<()> {
        let path = if delete_files { self.primary_file_path(episode_id).await } else { None };
        let mut active = self.active.lock().await;
        let mut files = self.files.lock().await;
        let Some(handle) = active.remove(&episode_id) else { return Ok(()) };
        files.remove(&episode_id);
        let shared = active.values().any(|h| h.info_hash() == handle.info_hash());
        if shared {
            let wanted = shared_files(&active, &files, &handle);
            drop((active, files));
            self.update_files(&handle, &wanted).await?;
            if let Some(path) = path {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Ok(());
        }
        drop((active, files));
        self.session.delete(TorrentIdOrHash::Hash(handle.info_hash()), delete_files).await?;
        Ok(())
    }
}

fn shared_files(
    active: &HashMap<i64, Arc<ManagedTorrent>>,
    files: &HashMap<i64, usize>,
    handle: &Arc<ManagedTorrent>,
) -> HashSet<usize> {
    active
        .iter()
        .filter(|(_, h)| h.info_hash() == handle.info_hash())
        .filter_map(|(id, _)| files.get(id).copied())
        .collect()
}
