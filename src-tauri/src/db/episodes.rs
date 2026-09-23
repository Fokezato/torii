use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Serialize, FromRow)]
pub struct Episode {
    pub id: i64,
    pub watch_id: i64,
    pub source_item_id: Option<String>,
    pub name: Option<String>,
    pub magnet_uri: Option<String>,
    pub info_hash: Option<String>,
    pub save_path: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub jellyfin_item_id: Option<String>,
    pub item_path: Option<String>,
    pub added_at: String,
    pub available_at: Option<String>,
    pub deleted_at: Option<String>,
    pub episode_number: Option<i64>,
    /// Onde o player parou da última vez (ms).
    pub watch_position_ms: Option<i64>,
    /// Quando chegou no encerramento (ou 90%) pela 1ª vez — "assistido".
    pub watched_at: Option<String>,
    /// Quando passou pelo "remover áudios extras" (ver `audio_strip`).
    pub audio_processed_at: Option<String>,
    /// Quando passou pelo "reduzir resolução" (ver `downscale`).
    pub video_processed_at: Option<String>,
    /// Última vez que o player salvou progresso (ordena o "Continuar assistindo").
    pub watch_progress_at: Option<String>,
}

pub struct NewEpisode<'a> {
    pub watch_id: i64,
    pub source_item_id: &'a str,
    pub name: &'a str,
    pub magnet_uri: &'a str,
    pub save_path: &'a str,
    pub status: &'a str,
    /// Número extraído do título ("SxxEyy"). Ver migração 0008: força 1 linha
    /// por (watch, episódio) mesmo quando 2 releases diferentes casam em
    /// polls separados — sem isso, source_item_id sozinho não pega esse caso
    /// e o mesmo episódio duplicava de novo (bug real reportado 2x).
    pub episode_number: Option<i64>,
}

/// Idempotente por design, protegido por 2 índices únicos: (watch_id,
/// source_item_id) da migração 0006 — a MESMA release não entra 2x — e
/// (watch_id, episode_number) da 0008 — releases DIFERENTES do MESMO
/// episódio (ex. poll 1 escolhe ToonsHub, poll 2 escolhe VARYG) também não
/// duplicam. `INSERT OR IGNORE` só ignora o conflito; sempre devolve a linha
/// que existe agora, nova ou não. Quem chama confere `status`: "found" =
/// inserção de verdade, dispara download; "error" = já existia mas a
/// tentativa anterior falhou, quem chama deve trocar a fonte (replace) em
/// vez de tratar como duplicata.
pub async fn add(pool: &SqlitePool, e: NewEpisode<'_>) -> Result<Episode, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO episodes (watch_id, source_item_id, name, magnet_uri, save_path, status, episode_number, added_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(e.watch_id)
    .bind(e.source_item_id)
    .bind(e.name)
    .bind(e.magnet_uri)
    .bind(e.save_path)
    .bind(e.status)
    .bind(e.episode_number)
    .bind(&now)
    .execute(pool)
    .await?;

    // Tenta pelo episode_number primeiro (cobre o caso comum). Mas o INSERT
    // pode ter sido ignorado por conflito no OUTRO índice único, o de
    // source_item_id (migração 0006) — contra uma linha pré-existente de
    // ANTES da 0008, que sempre tem episode_number NULL. Aí a busca por
    // episode_number não acha nada, e sem esse fallback isso virava
    // `fetch_one` vazio = erro, episódio inteiro silenciosamente descartado
    // do lote (bug real: Re:ZERO S3 sumia episódio sem nem logar).
    if let Some(episode_number) = e.episode_number {
        if let Some(row) = sqlx::query_as::<_, Episode>(
            "SELECT * FROM episodes WHERE watch_id = ? AND episode_number = ?",
        )
        .bind(e.watch_id)
        .bind(episode_number)
        .fetch_optional(pool)
        .await?
        {
            return Ok(row);
        }
    }

    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE watch_id = ? AND source_item_id = ?")
        .bind(e.watch_id)
        .bind(e.source_item_id)
        .fetch_one(pool)
        .await
}

/// Linha "placeholder" criada ao adicionar o anime na biblioteca, antes de
/// qualquer busca no Nyaa — o episódio já aparece na Biblioteca (status
/// "pending") em vez de só surgir quando (e se) o poller achar um torrent
/// pra ele. Resolve o bug recorrente de episódio "sumir" da Biblioteca: a
/// linha sempre existe, só o status muda enquanto o motor procura/baixa.
/// `INSERT OR IGNORE` pelo índice único (watch_id, episode_number) — chamar
/// de novo (ex. usuário recriando o watch) não duplica.
pub async fn create_placeholder(
    pool: &SqlitePool,
    watch_id: i64,
    save_path: &str,
    episode_number: i64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO episodes (watch_id, save_path, status, episode_number, added_at) \
         VALUES (?, ?, 'pending', ?, ?)",
    )
    .bind(watch_id)
    .bind(save_path)
    .bind(episode_number)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Episódio real criado ANTES da migração 0008 nunca teve `episode_number`
/// gravado (coluna só passou a existir depois). Sem numerar essas linhas
/// antigas, `create_placeholder` não as reconhece como "esse episódio já
/// existe" e cria um "pending" duplicado por cima (bug real: virou 30 linhas
/// pra um watch de 16 episódios na primeira vez que rodou o backfill).
/// Exclui "deleted": episódio real deletado DEPOIS da 0008 já nasce
/// numerado (o fluxo normal seta `episode_number` na criação, bem antes de
/// qualquer deleção) — só sobra aqui lixo de ANTES dessa feature (era da
/// duplicata), e numerar esse lixo travaria o slot pra sempre sem nenhuma
/// linha visível ocupando ele (outro jeito do episódio "sumir").
pub async fn list_missing_episode_number(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE episode_number IS NULL AND name IS NOT NULL AND status != 'deleted'",
    )
    .fetch_all(pool)
    .await
}

pub async fn set_episode_number(pool: &SqlitePool, id: i64, episode_number: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET episode_number = ? WHERE id = ?")
        .bind(episode_number)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Volta um episódio pro estado "pending" (não apaga a linha — ela é o
/// placeholder permanente do episódio na Biblioteca). Usado quando o
/// usuário cancela um download incompleto (botão X no Downloads): diferente
/// de `mark_deleted` (retenção — de propósito terminal, não deve ressurgir),
/// cancelar um download deve deixar o episódio "procurável" de novo.
pub async fn mark_pending(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE episodes SET status = 'pending', source_item_id = NULL, name = NULL, \
         magnet_uri = NULL, info_hash = NULL, item_path = NULL, jellyfin_item_id = NULL, \
         error_message = NULL, available_at = NULL, deleted_at = NULL \
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes ORDER BY added_at DESC LIMIT ?")
        .bind(limit)
        .fetch_all(pool)
        .await
}

/// Todos os episódios prontos pra assistir (base do "Continuar assistindo").
pub async fn list_available(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE status = 'available'")
        .fetch_all(pool)
        .await
}

pub async fn list_for_watch(pool: &SqlitePool, watch_id: i64) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE watch_id = ? ORDER BY added_at DESC")
        .bind(watch_id)
        .fetch_all(pool)
        .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Episode, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn list_by_status(pool: &SqlitePool, status: &str) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>("SELECT * FROM episodes WHERE status = ? ORDER BY added_at")
        .bind(status)
        .fetch_all(pool)
        .await
}

pub async fn set_item_path(pool: &SqlitePool, id: i64, item_path: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET item_path = ? WHERE id = ?")
        .bind(item_path)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_jellyfin_item_id(pool: &SqlitePool, id: i64, jellyfin_item_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET jellyfin_item_id = ? WHERE id = ?")
        .bind(jellyfin_item_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_downloading(pool: &SqlitePool, id: i64, info_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET status = 'downloading', info_hash = ?, error_message = NULL WHERE id = ?")
        .bind(info_hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Idempotente de propósito: o reconciler chama isso a cada tick enquanto o
/// torrent seguir rastreado (não paramos de rastrear ao terminar, pra
/// "remover" continuar funcionando depois), então `available_at` só deve
/// ser setado na PRIMEIRA vez, não reatualizado a cada 2s.
pub async fn mark_available(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE episodes SET status = 'available', available_at = COALESCE(available_at, ?) WHERE id = ?",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_error(pool: &SqlitePool, id: i64, message: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET status = 'error', error_message = ? WHERE id = ?")
        .bind(message)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Troca a fonte de um episódio já rastreado (menu "..." > Trocar fonte na
/// Biblioteca). Volta pro estado "found" do zero — quem chama ainda precisa
/// cancelar o torrent/arquivo antigo (`TorrentEngine::remove`) e iniciar o
/// novo download (`engine::start_download`) separadamente.
pub async fn switch_source(
    pool: &SqlitePool,
    id: i64,
    source_item_id: &str,
    name: &str,
    magnet_uri: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE episodes SET source_item_id = ?, name = ?, magnet_uri = ?, status = 'found', \
         info_hash = NULL, item_path = NULL, jellyfin_item_id = NULL, error_message = NULL, \
         available_at = NULL, deleted_at = NULL \
         WHERE id = ?",
    )
    .bind(source_item_id)
    .bind(name)
    .bind(magnet_uri)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Progresso do player. `watched` só MARCA (nunca desmarca): voltar pro
/// começo de um episódio já assistido não cancela a limpeza agendada.
pub async fn save_progress(
    pool: &SqlitePool,
    watch_id: i64,
    episode_number: i64,
    position_ms: i64,
    watched: bool,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE episodes SET watch_position_ms = ?, watch_progress_at = ?, \
         watched_at = CASE WHEN ? THEN COALESCE(watched_at, ?) ELSE watched_at END \
         WHERE watch_id = ? AND episode_number = ? AND status = 'available'",
    )
    .bind(position_ms)
    .bind(&now)
    .bind(watched)
    .bind(&now)
    .bind(watch_id)
    .bind(episode_number)
    .execute(pool)
    .await?;
    Ok(())
}

/// Prontos com arquivo e com alguma etapa do pós-processamento ainda não
/// feita (ver `postprocess`) — quem chama decide qual se aplica.
pub async fn list_postprocess_pending(pool: &SqlitePool) -> Result<Vec<Episode>, sqlx::Error> {
    sqlx::query_as::<_, Episode>(
        "SELECT * FROM episodes WHERE status = 'available' AND item_path IS NOT NULL \
         AND (audio_processed_at IS NULL OR video_processed_at IS NULL) ORDER BY available_at",
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_video_processed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET video_processed_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_audio_processed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE episodes SET audio_processed_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_deleted(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE episodes SET status = 'deleted', deleted_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
