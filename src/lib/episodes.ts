import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type EpisodeStatus = "pending" | "found" | "downloading" | "available" | "error" | "deleted";

export interface Episode {
  id: number;
  watch_id: number;
  source_item_id: string | null;
  name: string | null;
  magnet_uri: string | null;
  info_hash: string | null;
  save_path: string | null;
  status: EpisodeStatus;
  error_message: string | null;
  jellyfin_item_id: string | null;
  item_path: string | null;
  added_at: string;
  available_at: string | null;
  deleted_at: string | null;
  episode_number: number | null;
  /** Onde o player parou da última vez (ms). */
  watch_position_ms: number | null;
  /** Quando foi assistido até o encerramento/90% (ISO). */
  watched_at: string | null;
  /** Última vez que o player salvou progresso (ISO). */
  watch_progress_at: string | null;
}

export interface DownloadProgress {
  episode_id: number;
  state: "initializing" | "live" | "paused" | "error";
  progress_bytes: number;
  total_bytes: number;
  finished: boolean;
  download_speed_mbps: number | null;
  upload_speed_mbps: number | null;
  eta_human: string | null;
  peers: number | null;
}

export async function listRecentEpisodes(): Promise<Episode[]> {
  return invoke<Episode[]>("list_recent_episodes");
}

/** Todos os episódios prontos pra assistir, de todos os animes. */
export async function listAvailableEpisodes(): Promise<Episode[]> {
  return invoke<Episode[]>("list_available_episodes");
}

export async function listWatchEpisodes(watchId: number): Promise<Episode[]> {
  return invoke<Episode[]>("list_watch_episodes", { watchId });
}

export async function pauseEpisodeDownload(episodeId: number): Promise<void> {
  return invoke<void>("pause_episode_download", { episodeId });
}

export async function resumeEpisodeDownload(episodeId: number): Promise<void> {
  return invoke<void>("resume_episode_download", { episodeId });
}

export async function cancelEpisodeDownload(episodeId: number, deleteFiles: boolean): Promise<void> {
  return invoke<void>("cancel_episode_download", { episodeId, deleteFiles });
}

export interface EpisodeSource {
  id: number;
  episode_id: number;
  source_item_id: string;
  title: string;
  magnet_uri: string;
  seeders: number | null;
  leechers: number | null;
  size: string | null;
  is_active: number;
}

export async function listEpisodeSources(episodeId: number): Promise<EpisodeSource[]> {
  return invoke<EpisodeSource[]>("list_episode_sources", { episodeId });
}

export async function switchEpisodeSource(episodeId: number, sourceItemId: string): Promise<void> {
  return invoke<void>("switch_episode_source", { episodeId, sourceItemId });
}

/** true = achou e iniciou download; false = nenhum candidato dessa vez. */
export async function forceCheckEpisode(episodeId: number): Promise<boolean> {
  return invoke<boolean>("force_check_episode", { episodeId });
}

export function onDownloadProgress(callback: (progress: DownloadProgress[]) => void) {
  return listen<DownloadProgress[]>("downloads:progress", (event) => callback(event.payload));
}
