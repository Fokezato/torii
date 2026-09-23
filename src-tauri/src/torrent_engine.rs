use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session, TorrentStats};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;

/// Sem isso, um magnet sem peer respondendo (comum — nem todo torrent do
/// Nyaa tem seed saudável) trava `add_torrent` esperando metadata pra
/// sempre. Como quem chama processa vários episódios em sequência (ver
/// `poll_watch`), 1 magnet travado travava TODOS os episódios depois dele
/// na mesma leva — bug real reportado (Re:ZERO S3 parava sempre no mesmo
/// episódio, o resto nunca era tentado).
const ADD_TORRENT_TIMEOUT: Duration = Duration::from_secs(45);

pub struct TorrentEngine {
    session: Arc<Session>,
    // episode_id -> handle, pra reconciler poder pedir stats sem precisar
    // re-resolver por id/hash a cada tick.
    active: Mutex<HashMap<i64, Arc<ManagedTorrent>>>,
}

impl TorrentEngine {
    pub async fn new(default_output_folder: PathBuf) -> anyhow::Result<Self> {
        let session = Session::new(default_output_folder).await?;
        Ok(Self {
            session,
            active: Mutex::new(HashMap::new()),
        })
    }

    /// Adiciona um magnet e passa a rastrear ele sob o episode_id, devolvendo
    /// o info_hash real (pra correlação estável entre restarts, ao invés da
    /// convenção de string frágil que o app antigo em Python usava).
    pub async fn add_download(
        &self,
        episode_id: i64,
        magnet: &str,
        output_folder: &str,
    ) -> anyhow::Result<String> {
        let opts = AddTorrentOptions {
            output_folder: Some(output_folder.to_string()),
            overwrite: true,
            ..Default::default()
        };
        let response = tokio::time::timeout(
            ADD_TORRENT_TIMEOUT,
            self.session.add_torrent(AddTorrent::from_url(magnet), Some(opts)),
        )
        .await
        .map_err(|_| anyhow::anyhow!(tr!("timeout resolvendo metadata do torrent (sem peers respondendo)", "timed out resolving torrent metadata (no peers responding)")))??;
        let handle = response
            .into_handle()
            .ok_or_else(|| anyhow::anyhow!(tr!("torrent ficou list-only, sem handle pra rastrear", "torrent ended up list-only, no handle to track")))?;
        let info_hash = handle.info_hash().as_string();
        self.active.lock().await.insert(episode_id, handle);
        Ok(info_hash)
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

    /// Snapshot de progresso de tudo que tá sendo rastreado no momento. Quem
    /// chama decide o que fazer com itens já `finished` (ex. persistir status
    /// e parar de rastrear via `untrack`).
    pub async fn snapshot(&self) -> Vec<(i64, TorrentStats)> {
        let active = self.active.lock().await;
        active
            .iter()
            .map(|(episode_id, handle)| (*episode_id, handle.stats()))
            .collect()
    }

    /// Caminho completo do arquivo principal (maior arquivo) do torrent, se
    /// os metadados já resolveram. Usado pra renomear o arquivo baixado com
    /// um nome limpo assim que termina.
    pub async fn primary_file_path(&self, episode_id: i64) -> Option<PathBuf> {
        let handle = self.active.lock().await.get(&episode_id).cloned()?;
        let output_folder = handle.output_folder().to_path_buf();
        let meta = handle.metadata.load();
        let file_infos = &meta.as_ref()?.file_infos;
        let biggest = file_infos.iter().max_by_key(|f| f.len)?;
        Some(output_folder.join(&biggest.relative_filename))
    }

    /// Remove o torrent da sessão (para de baixar/semear) e opcionalmente
    /// apaga os arquivos já baixados. Usado tanto pra "cancelar" um download
    /// em andamento (delete_files=true) quanto pra "remover" um já concluído
    /// da lista sem mexer no arquivo (delete_files=false).
    pub async fn remove(&self, episode_id: i64, delete_files: bool) -> anyhow::Result<()> {
        let handle = self.active.lock().await.remove(&episode_id);
        if let Some(handle) = handle {
            let id = TorrentIdOrHash::Hash(handle.info_hash());
            self.session.delete(id, delete_files).await?;
        }
        Ok(())
    }
}
