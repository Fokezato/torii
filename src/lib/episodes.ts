import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type EpisodeStatus = "pending" | "found" | "ready" | "downloading" | "available" | "error" | "deleted";

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
  watch_position_ms: number | null;
  watched_at: string | null;
  watch_progress_at: string | null;
  /** File inside a season pack; null = single-episode torrent. */
  file_index: number | null;
}

/** Episodes downloaded from the same season pack share this key. */
export function packKey(ep: Episode): string | null {
  return ep.file_index != null && ep.source_item_id ? `${ep.watch_id}:${ep.source_item_id}` : null;
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

export async function downloadMissingEpisodes(watchId: number): Promise<number> {
  return invoke<number>("download_missing_episodes", { watchId });
}

export async function episodeStreamStart(episodeId: number): Promise<string> {
  return invoke<string>("episode_stream_start", { episodeId });
}

export async function episodePrefetchNext(watchId: number, episodeNumber: number): Promise<void> {
  return invoke<void>("episode_prefetch_next", { watchId, episodeNumber });
}

/** Deletes the episode's file and marks it removed: it is not downloaded again automatically. */
export async function setEpisodeWatched(episodeId: number, watched: boolean): Promise<void> {
  return invoke<void>("set_episode_watched", { episodeId, watched });
}

export async function deleteEpisode(episodeId: number): Promise<void> {
  return invoke<void>("delete_episode", { episodeId });
}

export async function forceCheckEpisode(episodeId: number): Promise<boolean> {
  return invoke<boolean>("force_check_episode", { episodeId });
}

export function onDownloadProgress(callback: (progress: DownloadProgress[]) => void) {
  return listen<DownloadProgress[]>("downloads:progress", (event) => callback(event.payload));
}
