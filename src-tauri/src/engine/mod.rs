use crate::{db, jellyfin, notify, sources::nyaa, state::AppState};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Nome limpo pra notificação: "Nome Base — Episódio N", sem tag de
/// qualidade/codec/grupo do release cru (que quebra o layout da janela de
/// notificação, que é pequena de propósito). Cai pro nome base sozinho se
/// não achar número de episódio reconhecível no título cru.
fn notify_episode_title(watch_title: &str, raw_episode_title: &str) -> String {
    let (base, _) = nyaa::split_season(watch_title);
    match nyaa::extract_episode_number(raw_episode_title) {
        Some(ep) => tr!("{base} — Episódio {ep}", "{base} — Episode {ep}"),
        None => base,
    }
}

/// Pede pro engine de torrent baixar um episódio já salvo no banco (status
/// "found") e reflete o resultado de volta no banco + no log de atividade.
/// Reusado tanto no fluxo normal (achou candidato novo) quanto na
/// reconciliação de boot (episódios que ficaram "found"/"downloading" de uma
/// sessão anterior, já que o `TorrentEngine` não lembra nada entre restarts).
/// `notify_found`: manda ou não a notificação individual de "achou 1
/// episódio". `poll_watch` passa `false` e manda uma notificação só,
/// resumida, quando o lote tem mais de um episódio — sem isso, uma busca
/// que acha 20 episódios de uma vez (comum ao adicionar um anime que já tá
/// no ar há tempo) empilha 20 notificações de 5s cada, "travando" a janela
/// por 100s+. `resume_pending_downloads` (boot, poucos itens) passa `true`.
pub async fn start_download(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    watch_title: &str,
    title: &str,
    magnet: &str,
    folder: &str,
    cover_url: Option<String>,
    notify_found: bool,
) {
    let clean_title = notify_episode_title(watch_title, title);
    match state.torrent.add_download(episode_id, magnet, folder).await {
        Ok(info_hash) => {
            if let Err(e) = db::episodes::mark_downloading(&state.db, episode_id, &info_hash).await {
                state.activity.error(tr!("Erro ao salvar estado de download de \"{clean_title}\": {e}", "Failed to save download state of \"{clean_title}\": {e}"));
                return;
            }
            state.activity.info(format!("Iniciado download: {clean_title}"));
            if notify_found {
                notify::notify(
                    app,
                    state,
                    "notify_found",
                    tr!("Novo episódio encontrado", "New episode found"),
                    tr!("{clean_title} — iniciando download", "{clean_title} — starting download"),
                    "info",
                    cover_url,
                )
                .await;
            }
        }
        Err(e) => {
            let _ = db::episodes::mark_error(&state.db, episode_id, &e.to_string()).await;
            state.activity.error(tr!("Falha ao iniciar download de \"{clean_title}\": {e}", "Failed to start download of \"{clean_title}\": {e}"));
            notify::notify(
                app,
                state,
                "notify_error",
                tr!("Falha no download", "Download failed"),
                tr!("Não foi possível baixar {clean_title}", "Couldn't download {clean_title}"),
                "error",
                cover_url,
            )
            .await;
        }
    }
}

pub(crate) fn split_langs(raw: &Option<String>) -> Vec<String> {
    raw.as_deref()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub async fn poll_watch(app: &AppHandle, state: &AppState, watch: &db::watches::Watch) {
    let audio_langs = split_langs(&watch.audio_lang);
    let sub_langs = split_langs(&watch.sub_lang);

    let seen_ids = match db::seen_items::get_seen_ids(&state.db, watch.id).await {
        Ok(ids) => ids,
        Err(e) => {
            state.activity.error(format!("Erro ao checar \"{}\": {e}", watch.title));
            return;
        }
    };

    state.activity.info(tr!("Procurando episódios de \"{}\"...", "Searching episodes of \"{}\"...", watch.title));

    let result = nyaa::find_new_matches(
        &state.http,
        &watch.query,
        &watch.quality,
        &audio_langs,
        &sub_langs,
        watch.episode_start,
        watch.episode_end,
        &seen_ids,
    )
    .await;

    let result = match result {
        Ok(r) => r,
        Err(e) => {
            state.activity.error(format!("Falha ao procurar \"{}\": {e}", watch.title));
            return;
        }
    };

    // Candidato SEM match marca visto já aqui — não precisa de nenhum
    // processamento, seguro esquecer pra sempre. Candidato COM match só
    // marca visto depois de tentar processar (dentro do loop abaixo) — se o
    // app cair/reiniciar no meio do lote, ver comentário lá.
    for candidate in &result.all_new {
        let is_matched = result
            .matched
            .iter()
            .any(|m| m.primary.id == candidate.id || m.alternates.iter().any(|a| a.id == candidate.id));
        if !is_matched {
            let _ = db::seen_items::mark_seen(&state.db, watch.id, &candidate.id, &candidate.title, false).await;
        }
    }

    if result.matched.is_empty() {
        if !result.all_new.is_empty() {
            state.activity.info(tr!("Nada compatível ainda pra \"{}\"", "Nothing matching yet for \"{}\"", watch.title));
        }
        return;
    }

    // Achar 1 episódio novo = notificação individual de sempre. Achar vários
    // de uma vez (comum ao adicionar um anime que já tá no ar há tempo, ou
    // reprocessar depois de limpar o ledger) manda notificação por episódio
    // achado, que empilha uma fila de 5s cada — "trava" a janela por minutos.
    // Em lote, manda notificação por item MAS resume numa só no final.
    let batch = result.matched.len() > 1;
    let mut started = 0u32;

    for episode_match in &result.matched {
        let candidate = &episode_match.primary;

        // Marca visto só agora, ao alcançar o item no loop — não antes, em
        // lote, pra um episódio nunca-tentado (loop interrompido por um
        // anterior travado/reinício do app) não ficar "visto" pra sempre
        // sem nunca ter virado download de verdade (bug real: Re:ZERO S3
        // sempre parava no mesmo episódio e o resto nunca era retentado).
        let _ = db::seen_items::mark_seen(&state.db, watch.id, &candidate.id, &candidate.title, true).await;
        for alt in &episode_match.alternates {
            let _ = db::seen_items::mark_seen(&state.db, watch.id, &alt.id, &alt.title, true).await;
        }

        let episode_number = nyaa::extract_episode_number(&candidate.title).map(i64::from);
        let outcome = db::episodes::add(
            &state.db,
            db::episodes::NewEpisode {
                watch_id: watch.id,
                source_item_id: &candidate.id,
                name: &candidate.title,
                magnet_uri: &candidate.magnet,
                save_path: &watch.folder,
                status: "found",
                episode_number,
            },
        )
        .await;

        match outcome {
            // "pending" (placeholder criado ao adicionar o anime, ver
            // `create_placeholder`), "found" (linha nova) ou "error"
            // (tentativa anterior falhou) — todos elegíveis pra associar
            // essa fonte e (re)iniciar. "downloading"/"available" já tão
            // resolvidos, "deleted" é terminal (retenção) — não mexe.
            Ok(episode) if matches!(episode.status.as_str(), "pending" | "found" | "error") => {
                let clean_title = notify_episode_title(&watch.title, &candidate.title);
                if episode.status == "error" {
                    // Já existia linha desse episódio, mas a tentativa
                    // anterior (outra fonte, talvez a mesma) deu erro —
                    // troca a fonte em vez de empilhar linha nova (bug real
                    // reportado: 2 linhas de "Episódio 4", ambas com erro).
                    state.activity.info(tr!("Tentando de novo: {clean_title}", "Retrying: {clean_title}"));
                } else {
                    state.activity.info(format!("Encontrado: {clean_title}"));
                }
                if let Err(e) = db::episodes::switch_source(
                    &state.db,
                    episode.id,
                    &candidate.id,
                    &candidate.title,
                    &candidate.magnet,
                )
                .await
                {
                    state.activity.error(tr!("Erro ao salvar fonte de \"{clean_title}\": {e}", "Failed to save source of \"{clean_title}\": {e}"));
                    continue;
                }
                if let Err(e) =
                    db::episode_sources::add_many(&state.db, episode.id, candidate, &episode_match.alternates)
                        .await
                {
                    state.activity.error(tr!("Erro ao salvar fontes de \"{clean_title}\": {e}", "Failed to save sources of \"{clean_title}\": {e}"));
                }
                let _ = db::episode_sources::set_active(&state.db, episode.id, &candidate.id).await;
                start_download(
                    app,
                    state,
                    episode.id,
                    &watch.title,
                    &candidate.title,
                    &candidate.magnet,
                    &watch.folder,
                    watch.cover_url.clone(),
                    !batch,
                )
                .await;
                started += 1;
            }
            Ok(episode) if episode.status == "downloading" => {
                let candidates: Vec<&nyaa::NyaaCandidate> =
                    std::iter::once(candidate).chain(episode_match.alternates.iter()).collect();
                let _ = db::episode_sources::add_alternates(&state.db, episode.id, &candidates).await;
            }
            Ok(_) => {}
            Err(e) => state
                .activity
                .error(tr!("Erro ao salvar episódio de \"{}\": {e}", "Failed to save episode of \"{}\": {e}", watch.title)),
        }
    }

    if batch && started > 0 {
        let (base_title, _) = nyaa::split_season(&watch.title);
        notify::notify(
            app,
            state,
            "notify_found",
            tr!("Novos episódios encontrados", "New episodes found"),
            tr!("{started} episódios de {base_title} — iniciando download", "{started} episodes of {base_title} — starting download"),
            "info",
            watch.cover_url.clone(),
        )
        .await;
    }
}

/// Roda uma vez no boot: o `TorrentEngine` é uma sessão nova a cada start do
/// app (sem persistência própria ainda), então qualquer episódio que ficou
/// "found" (nunca chegou a iniciar) ou "downloading" (app fechou no meio) de
/// uma sessão anterior precisa ser re-adicionado pra voltar a ser rastreado.
pub async fn resume_pending_downloads(app: AppHandle) {
    let app = &app;
    let state = app.state::<AppState>();
    let mut pending = Vec::new();
    for status in ["found", "downloading"] {
        match db::episodes::list_by_status(&state.db, status).await {
            Ok(mut eps) => pending.append(&mut eps),
            Err(e) => state.activity.error(tr!("Erro ao listar episódios pendentes: {e}", "Failed to list pending episodes: {e}")),
        }
    }

    // Mesma lógica de lote do poll_watch: reconciliar vários pendentes de
    // uma vez no boot não pode empilhar uma notificação por item.
    let notify_individually = pending.len() <= 1;

    for ep in pending {
        let (Some(magnet), Some(save_path)) = (ep.magnet_uri.as_deref(), ep.save_path.as_deref()) else {
            continue;
        };
        let title = ep.name.as_deref().unwrap_or("episódio");
        let Ok(watch) = db::watches::get(&state.db, ep.watch_id).await else {
            continue;
        };
        start_download(
            app,
            &state,
            ep.id,
            &watch.title,
            title,
            magnet,
            save_path,
            watch.cover_url,
            notify_individually,
        )
        .await;
    }
}

/// Renomeia o arquivo baixado pra "Nome do Anime S0NE0M Título do
/// Episódio.ext", descartando tag de qualidade/codec/grupo do release —
/// deixa o disco organizado igual um app de streaming, e ajuda o Jellyfin
/// a casar o episódio certo. Best-effort: qualquer falha só loga, não
/// impede o episódio de ser marcado "available". Devolve o path final do
/// arquivo (renomeado ou original, se o rename não rolou) pra quem chama
/// poder usar na sincronização com o Jellyfin.
async fn rename_to_clean_filename(state: &AppState, episode_id: i64, watch_id: i64) -> Option<PathBuf> {
    let old_path = state.torrent.primary_file_path(episode_id).await?;
    let Ok(watch) = db::watches::get(&state.db, watch_id).await else {
        return Some(old_path);
    };
    let (base_title, _) = nyaa::split_season(&watch.title);
    let Some(raw_filename) = old_path.file_name().and_then(|n| n.to_str()) else {
        return Some(old_path);
    };
    let Some(clean_name) = crate::clean_filename::clean_episode_filename(&base_title, raw_filename) else {
        return Some(old_path);
    };
    let new_path = old_path.with_file_name(&clean_name);
    if new_path == old_path {
        return Some(old_path);
    }
    match tokio::fs::rename(&old_path, &new_path).await {
        Ok(()) => {
            let _ = db::episodes::set_item_path(&state.db, episode_id, &new_path.to_string_lossy()).await;
            Some(new_path)
        }
        Err(e) => {
            state.activity.error(format!("Erro ao renomear arquivo baixado: {e}"));
            Some(old_path)
        }
    }
}

/// Pede refresh da pasta pro Jellyfin e tenta achar o item correspondente
/// pra guardar o `jellyfin_item_id` (usado depois pra deletar certo na
/// limpeza por retenção). Roda em background separado do reconciler porque
/// o Jellyfin pode levar até uns 30s pra terminar de escanear.
async fn sync_jellyfin_after_download(app: AppHandle, episode_id: i64, file_path: PathBuf) {
    let state = app.state::<AppState>();
    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if settings.get("jellyfin_mode").map(String::as_str) != Some("1") {
        return;
    }
    let (Some(url), Some(api_key)) = (
        settings.get("jellyfin_url").filter(|s| !s.is_empty()),
        settings.get("jellyfin_api_key").filter(|s| !s.is_empty()),
    ) else {
        return;
    };

    let parent = file_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if let Err(e) = jellyfin::refresh_path(&state.http, url, api_key, &parent).await {
        state.activity.error(format!("Erro ao pedir refresh ao Jellyfin: {e}"));
        return;
    }

    let file_path_str = file_path.to_string_lossy().to_string();
    match jellyfin::find_item_by_path(&state.http, url, api_key, &file_path_str).await {
        Ok(Some(item_id)) => {
            let _ = db::episodes::set_jellyfin_item_id(&state.db, episode_id, &item_id).await;
            if let Ok(episode) = db::episodes::get(&state.db, episode_id).await {
                let raw_title = episode.name.clone().unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
                if let Ok(watch) = db::watches::get(&state.db, episode.watch_id).await {
                    let clean_title = notify_episode_title(&watch.title, &raw_title);
                    notify::notify(
                        &app,
                        &state,
                        "notify_jellyfin",
                        tr!("Disponível no Jellyfin", "Available on Jellyfin"),
                        clean_title,
                        "success",
                        watch.cover_url,
                    )
                    .await;
                }
            }
        }
        Ok(None) => {
            state.activity.error(tr!("Jellyfin não achou o episódio depois do refresh", "Jellyfin didn't find the episode after the refresh"));
        }
        Err(e) => {
            state.activity.error(format!("Erro ao consultar Jellyfin: {e}"));
        }
    }
}

/// Roda uma vez no boot: episódios que já ficaram "available" sem um
/// `jellyfin_item_id" (ex. baixados antes da integração Jellyfin existir,
/// ou que a primeira tentativa de match falhou) tentam de novo.
pub async fn resync_jellyfin_library(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(episodes) = db::episodes::list_by_status(&state.db, "available").await else {
        return;
    };
    let pending: Vec<(i64, String)> = episodes
        .into_iter()
        .filter(|e| e.jellyfin_item_id.is_none())
        .filter_map(|e| e.item_path.map(|p| (e.id, p)))
        .collect();
    drop(state);

    for (episode_id, item_path) in pending {
        sync_jellyfin_after_download(app.clone(), episode_id, PathBuf::from(item_path)).await;
    }
}

/// Consulta o progresso de todo torrent rastreado a cada tick, emite pro
/// front via evento, e persiste quando um episódio termina de baixar.
/// Cancela o torrent atual do episódio (apagando o parcial) e recomeça pela
/// fonte `source_item_id`, que tem que estar em `episode_sources`.
pub async fn switch_source(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    source_item_id: &str,
) -> Result<(), crate::error::AppError> {
    let source = db::episode_sources::get(&state.db, episode_id, source_item_id).await?;
    let _ = state.torrent.remove(episode_id, true).await;
    db::episodes::switch_source(&state.db, episode_id, &source.source_item_id, &source.title, &source.magnet_uri)
        .await?;
    db::episode_sources::set_active(&state.db, episode_id, source_item_id).await?;

    let episode = db::episodes::get(&state.db, episode_id).await?;
    let watch = db::watches::get(&state.db, episode.watch_id).await?;
    start_download(
        app,
        state,
        episode_id,
        &watch.title,
        &source.title,
        &source.magnet_uri,
        &watch.folder,
        watch.cover_url,
        true,
    )
    .await;
    Ok(())
}

/// Tempo sem receber nenhum byte pra considerar o download travado. A
/// contagem de seeds do Nyaa costuma estar desatualizada: torrent "com 11
/// seeds" que ninguém mais semeia de verdade (confirmado no qBittorrent).
const STALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Progresso visto por episódio: (bytes, quando mudou pela última vez).
type StallTracker = std::collections::HashMap<i64, (u64, std::time::Instant)>;

/// Download travado → troca pra melhor fonte alternativa ainda não tentada.
/// `tried` guarda as fontes já usadas nesta sessão pra não ficar em ciclo.
async fn handle_stalled(
    app: &AppHandle,
    state: &AppState,
    episode_id: i64,
    tried: &mut std::collections::HashMap<i64, std::collections::HashSet<String>>,
) {
    let Ok(sources) = db::episode_sources::list(&state.db, episode_id).await else {
        return;
    };
    let tried_here = tried.entry(episode_id).or_default();
    for s in sources.iter().filter(|s| s.is_active == 1) {
        tried_here.insert(s.source_item_id.clone());
    }
    // `list` já vem ordenado por seeds (maior primeiro).
    let Some(next) = sources.iter().find(|s| !tried_here.contains(&s.source_item_id)) else {
        return;
    };
    tried_here.insert(next.source_item_id.clone());
    let name = db::episodes::get(&state.db, episode_id)
        .await
        .ok()
        .and_then(|e| e.name)
        .unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
    state.activity.info(tr!(
        "Download parado há 10 min, trocando de fonte: {name}",
        "Download stalled for 10 min, switching source: {name}"
    ));
    if let Err(e) = switch_source(app, state, episode_id, &next.source_item_id).await {
        state.activity.error(tr!("Erro ao trocar de fonte: {e}", "Failed to switch source: {e}"));
    }
}

pub fn spawn_download_reconciler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut stall: StallTracker = StallTracker::new();
        let mut tried = std::collections::HashMap::new();
        loop {
            {
                let state = app.state::<AppState>();
                let snapshot = state.torrent.snapshot().await;

                // Travado = rodando (não pausado/terminado) e sem byte novo
                // há STALL_TIMEOUT. Pausa manual zera a contagem.
                let now = std::time::Instant::now();
                let mut stalled = Vec::new();
                stall.retain(|id, _| snapshot.iter().any(|(e, _)| e == id));
                for (episode_id, stats) in &snapshot {
                    let running = !stats.finished && stats.state.to_string() == "live";
                    let entry = stall.entry(*episode_id).or_insert((stats.progress_bytes, now));
                    if !running || stats.progress_bytes != entry.0 {
                        *entry = (stats.progress_bytes, now);
                    } else if now.duration_since(entry.1) >= STALL_TIMEOUT {
                        stalled.push(*episode_id);
                        *entry = (stats.progress_bytes, now);
                    }
                }
                for episode_id in stalled {
                    handle_stalled(&app, &state, episode_id, &mut tried).await;
                }

                let payload: Vec<serde_json::Value> = snapshot
                    .iter()
                    .map(|(episode_id, stats)| {
                        let live = stats.live.as_ref();
                        serde_json::json!({
                            "episode_id": episode_id,
                            "state": stats.state.to_string(),
                            "progress_bytes": stats.progress_bytes,
                            "total_bytes": stats.total_bytes,
                            "finished": stats.finished,
                            "download_speed_mbps": live.map(|l| l.download_speed.mbps),
                            "upload_speed_mbps": live.map(|l| l.upload_speed.mbps),
                            "eta_human": live.and_then(|l| l.time_remaining.as_ref()).map(|d| d.to_string()),
                            "peers": live.map(|l| l.snapshot.peer_stats.live),
                        })
                    })
                    .collect();
                let _ = app.emit("downloads:progress", payload);

                for (episode_id, stats) in snapshot {
                    // Continua rastreado após terminar (pra "remover" funcionar
                    // depois), então isso roda de novo a cada tick — só avisa
                    // e persiste na primeira vez que detecta a transição.
                    if stats.finished {
                        let Ok(episode) = db::episodes::get(&state.db, episode_id).await else {
                            continue;
                        };
                        if episode.status == "downloading" {
                            let title = episode.name.clone().unwrap_or_else(|| tr!("episódio #{episode_id}", "episode #{episode_id}"));
                            let watch = db::watches::get(&state.db, episode.watch_id).await.ok();
                            let cover_url = watch.as_ref().and_then(|w| w.cover_url.clone());
                            let clean_title = watch
                                .as_ref()
                                .map(|w| notify_episode_title(&w.title, &title))
                                .unwrap_or_else(|| title.clone());
                            let final_path = rename_to_clean_filename(&state, episode_id, episode.watch_id).await;
                            if let Err(e) = db::episodes::mark_available(&state.db, episode_id).await {
                                state.activity.error(tr!("Erro ao marcar episódio disponível: {e}", "Failed to mark episode as available: {e}"));
                            } else {
                                state.activity.info(tr!("Download concluído: {clean_title}", "Download complete: {clean_title}"));
                                notify::notify(
                                    &app,
                                    &state,
                                    "notify_complete",
                                    tr!("Download concluído", "Download complete"),
                                    tr!("{clean_title} já está pronto pra assistir", "{clean_title} is ready to watch"),
                                    "success",
                                    cover_url,
                                )
                                .await;
                            }
                            crate::intro_detect::spawn_pending(&app);
                            if let Some(path) = final_path {
                                tauri::async_runtime::spawn(sync_jellyfin_after_download(
                                    app.clone(),
                                    episode_id,
                                    path,
                                ));
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

pub async fn poll_once(app: &AppHandle, state: &AppState) {
    let Ok(_guard) = state.poll_lock.try_lock() else {
        return;
    };

    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if settings.get("paused").map(String::as_str) == Some("1") {
        return;
    }

    let watches = match db::watches::list(&state.db).await {
        Ok(w) => w,
        Err(_) => return,
    };

    for watch in watches.into_iter().filter(|w| w.active) {
        poll_watch(app, state, &watch).await;
    }
}

/// Remove um episódio de vez: tira do motor de torrent ANTES de apagar o
/// arquivo (o torrent segue rastreado/semeando depois de terminar — apagar
/// só o arquivo fazia o librqbit baixar de novo o que sumiu), apaga o
/// arquivo, remove do Jellyfin se configurado e marca "deleted" — status
/// terminal, o poller nunca mais baixa esse episódio (ver `poll_watch`).
async fn remove_episode(
    state: &AppState,
    ep: &db::episodes::Episode,
    clean_title: &str,
    jellyfin: Option<(&String, &String)>,
) -> bool {
    let _ = state.torrent.remove(ep.id, false).await;

    if let Some(path) = &ep.item_path {
        if let Err(e) = tokio::fs::remove_file(path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                state.activity.error(tr!("Erro ao apagar arquivo de \"{clean_title}\": {e}", "Failed to delete file of \"{clean_title}\": {e}"));
                return false;
            }
        }
    }

    if let (Some((url, api_key)), Some(item_id)) = (jellyfin, &ep.jellyfin_item_id) {
        if let Err(e) = jellyfin::delete_item(&state.http, url, api_key, item_id).await {
            state.activity.error(tr!("Erro ao remover \"{clean_title}\" do Jellyfin: {e}", "Failed to remove \"{clean_title}\" from Jellyfin: {e}"));
        }
    }

    if let Err(e) = db::episodes::mark_deleted(&state.db, ep.id).await {
        state.activity.error(format!("Erro ao marcar \"{clean_title}\" como removido: {e}"));
        return false;
    }
    true
}

/// Apaga episódios "available" que:
/// - passaram do tempo de retenção (`delete_after_days` do watch, ou o
///   default global) — sem retenção configurada = nunca por esse motivo;
/// - ou foram assistidos há mais de `delete_after_watched_hours`, se
///   "apagar depois de assistir" estiver ligado.
///
/// `playing_path`: arquivo aberto no player agora — nunca é apagado (no
/// Windows nem daria, arquivo em uso; e seria apagar no meio da sessão).
pub async fn cleanup_once(state: &AppState, playing_path: Option<String>) {
    let settings = match db::settings::get_all(&state.db).await {
        Ok(s) => s,
        Err(_) => return,
    };
    let global_default: Option<i64> = settings
        .get("default_delete_after_days")
        .and_then(|v| v.parse().ok());
    let delete_after_watched = settings.get("delete_after_watched").map(String::as_str) == Some("1");
    let watched_grace_hours: i64 = settings
        .get("delete_after_watched_hours")
        .and_then(|v| v.parse().ok())
        .unwrap_or(24)
        .max(0);
    let jellyfin_mode = settings.get("jellyfin_mode").map(String::as_str) == Some("1");
    let jellyfin_url = settings.get("jellyfin_url").filter(|s| !s.is_empty());
    let jellyfin_api_key = settings.get("jellyfin_api_key").filter(|s| !s.is_empty());
    let jellyfin = match (jellyfin_mode, jellyfin_url, jellyfin_api_key) {
        (true, Some(url), Some(key)) => Some((url, key)),
        _ => None,
    };

    let available = match db::episodes::list_by_status(&state.db, "available").await {
        Ok(e) => e,
        Err(_) => return,
    };
    let now = chrono::Utc::now();
    let parse = |s: Option<&str>| {
        s.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
    };

    for ep in available {
        if playing_path.is_some() && ep.item_path == playing_path {
            continue;
        }
        let Ok(watch) = db::watches::get(&state.db, ep.watch_id).await else {
            continue;
        };

        // <= 0 não é retenção válida (apagaria no ciclo seguinte à
        // disponibilidade) — trata igual a "sem retenção configurada".
        let retention = watch.delete_after_days.or(global_default).filter(|d| *d > 0);
        let expired_days = match (retention, parse(ep.available_at.as_deref())) {
            (Some(days), Some(available_at)) if (now - available_at).num_days() >= days => Some(days),
            _ => None,
        };
        let watched_long_ago = delete_after_watched
            && parse(ep.watched_at.as_deref())
                .is_some_and(|watched_at| (now - watched_at).num_hours() >= watched_grace_hours);
        if expired_days.is_none() && !watched_long_ago {
            continue;
        }

        let title = ep.name.clone().unwrap_or_else(|| tr!("episódio #{}", "episode #{}", ep.id));
        let clean_title = notify_episode_title(&watch.title, &title);
        if remove_episode(state, &ep, &clean_title, jellyfin).await {
            // Log interno só (aba de atividade) — sem notificação, de propósito.
            match expired_days {
                Some(days) => state.activity.info(tr!("Removido por retenção ({days}d): {clean_title}", "Removed by retention ({days}d): {clean_title}")),
                None => state.activity.info(tr!("Removido depois de assistido: {clean_title}", "Removed after watching: {clean_title}")),
            }
        }
    }
}

/// Roda 1x no boot: preenche placeholder ("pending") pra todo episódio da
/// temporada (1..=`episodes`) que ainda não tem linha — cobre watch criado
/// ANTES dessa feature existir e o intervalo escolhido (antes só o trecho
/// escolhido virava linha, e o resto sumia da Biblioteca — agora aparece
/// como "Não baixado"; o poller continua só buscando dentro do intervalo).
/// `create_placeholder` é idempotente (INSERT OR IGNORE por
/// watch_id+episode_number), não mexe em episódio que já tem release.
pub async fn backfill_placeholder_episodes(app: AppHandle) {
    let state = app.state::<AppState>();

    // Numera episódio real de ANTES da migração 0008 primeiro — sem isso o
    // passo abaixo não reconhece a linha como "já existe" e cria um pending
    // duplicado por cima (bug real, pego ainda em dev antes de afetar
    // ninguém: watch de 16 episódios virou 30 linhas na 1ª versão disso).
    if let Ok(unnumbered) = db::episodes::list_missing_episode_number(&state.db).await {
        for ep in unnumbered {
            if let Some(n) = ep.name.as_deref().and_then(nyaa::extract_episode_number) {
                let _ = db::episodes::set_episode_number(&state.db, ep.id, n as i64).await;
            }
        }
    }

    let Ok(watches) = db::watches::list(&state.db).await else {
        return;
    };
    for watch in watches {
        let Some(total) = watch.episodes.filter(|n| *n > 0) else {
            continue;
        };
        for ep in 1..=total {
            let _ = db::episodes::create_placeholder(&state.db, watch.id, &watch.folder, ep).await;
        }
    }
}

/// Roda 1x no boot: descobre o anime (franquia na AniList) das temporadas
/// que ainda não têm — cobre as adicionadas antes do agrupamento por
/// franquia existir. Só atualiza banco; não move pasta nem arquivo.
pub async fn backfill_series(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(watches) = db::watches::list(&state.db).await else {
        return;
    };
    for watch in watches.into_iter().filter(|w| w.series_anilist_id.is_none()) {
        let Some(anilist_id) = watch.anilist_id else { continue };
        if let Some((series_id, series_title)) =
            crate::commands::watches::resolve_series(&state.http, anilist_id).await
        {
            let _ = db::watches::set_series(&state.db, watch.id, series_id, &series_title).await;
        }
    }
}

pub async fn poll_watch_by_id(app: &AppHandle, watch_id: i64) {
    let state = app.state::<AppState>();
    if let Ok(watch) = db::watches::get(&state.db, watch_id).await {
        poll_watch(app, &state, &watch).await;
    }
}

/// Arquivo aberto no player nativo AGORA (tocando/pausado/carregando) —
/// `NowPlaying.source` sozinho continua com o último arquivo mesmo depois
/// do player parar, e travaria a limpeza desse episódio pra sempre.
fn currently_open_media(app: &AppHandle) -> Option<String> {
    use crate::player::ffi::VlcState;
    let player = app.state::<crate::player::PlayerState>();
    let state = player.engine.lock().unwrap().as_ref().map(|e| e.snapshot().state)?;
    if !matches!(state, VlcState::Playing | VlcState::Paused | VlcState::Buffering | VlcState::Opening) {
        return None;
    }
    let source = player.now_playing.lock().unwrap().source.clone();
    (!source.is_empty()).then_some(source)
}

pub fn spawn_background_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            {
                let state = app.state::<AppState>();
                poll_once(&app, &state).await;
                let playing = currently_open_media(&app);
                cleanup_once(&state, playing.clone()).await;
                crate::postprocess::spawn_pending(&app, playing);
            }

            let interval_minutes: u64 = {
                let state = app.state::<AppState>();
                db::settings::get_all(&state.db)
                    .await
                    .ok()
                    .and_then(|s| s.get("poll_interval_minutes").cloned())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30)
            };
            let secs = (interval_minutes * 60).max(60);
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_episode_title_strips_release_junk() {
        // Caso real que quebrava a janela de notificação (nome cru todo
        // socado num espaço de ~340px sem quebra de linha adequada).
        assert_eq!(
            notify_episode_title(
                "Re:ZERO -Starting Life in Another World- Season 4",
                "[ToonsHub] ReZERO -Starting Life in Another World- S04E01 1080p CR WEB-DL MULTi AAC2.0 H.264 (Re:Zero kara Hajimeru Isekai Seikatsu, Multi-Audio, Multi-Subs)",
            ),
            "Re:ZERO -Starting Life in Another World- — Episódio 1"
        );
    }

    #[test]
    fn notify_episode_title_falls_back_to_base_without_episode_number() {
        assert_eq!(notify_episode_title("Show Season 2", "Some batch release with no episode tag"), "Show");
    }
}
