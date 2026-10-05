import { invoke } from "@tauri-apps/api/core";
import { getSeriesGroupKey, parseSeasonFromTitle, stripSeasonSuffix } from "@/lib/episodeName";

export interface Watch {
  id: number;
  title: string;
  query: string;
  anilist_id: number | null;
  cover_url: string | null;
  quality: string;
  audio_lang: string | null;
  sub_lang: string | null;
  folder: string;
  delete_after_days: number | null;
  rating: number | null;
  active: boolean;
  notify_on_available: boolean;
  status: string | null;
  list_status: string;
  episodes: number | null;
  episode_start: number | null;
  episode_end: number | null;
  created_at: string;
  updated_at: string;
  max_resolution: string | null;
  strip_audio: boolean;
  series_anilist_id: number | null;
  series_title: string | null;
  streaming: boolean;
  /** null = follow the global setting. */
  delete_after_watched: boolean | null;
  /** AniList format: TV, MOVIE, OVA... */
  format: string | null;
}

export function seriesKeyOf(w: Watch): string {
  return w.series_anilist_id != null ? `anilist:${w.series_anilist_id}` : `title:${getSeriesGroupKey(w.title)}`;
}

export function seriesTitleOf(w: Watch): string {
  return w.series_title ?? stripSeasonSuffix(w.title);
}

export interface WatchGroup {
  key: string;
  title: string;
  representative: Watch;
  seasons: Watch[];
  updated_at: string;
}

export function groupBySeries(watches: Watch[]): WatchGroup[] {
  const map = new Map<string, Watch[]>();
  for (const w of watches) {
    const key = seriesKeyOf(w);
    const arr = map.get(key);
    if (arr) arr.push(w);
    else map.set(key, [w]);
  }
  return Array.from(map.entries()).map(([key, seasons]) => {
    const sorted = [...seasons].sort(
      (a, b) =>
        (parseSeasonFromTitle(b.title) ?? 0) - (parseSeasonFromTitle(a.title) ?? 0) ||
        (b.anilist_id ?? 0) - (a.anilist_id ?? 0),
    );
    const updated_at = seasons.map((s) => s.updated_at).reduce((a, b) => (new Date(b) > new Date(a) ? b : a));
    return { key, title: seriesTitleOf(sorted[0]), representative: sorted[0], seasons: sorted, updated_at };
  });
}

export type ListStatus = "watching" | "downloaded" | "completed" | "planning";

export interface NewWatch {
  title: string;
  query: string;
  anilist_id?: number | null;
  cover_url?: string | null;
  quality?: string | null;
  audio_lang?: string | null;
  sub_lang?: string | null;
  folder?: string | null;
  delete_after_days?: number | null;
  active?: boolean | null;
  notify_on_available?: boolean | null;
  status?: string | null;
  list_status?: string | null;
  episodes?: number | null;
  episode_start?: number | null;
  episode_end?: number | null;
  max_resolution?: string | null;
  strip_audio?: boolean | null;
  streaming?: boolean | null;
  format?: string | null;
}

export async function listWatches(): Promise<Watch[]> {
  return invoke<Watch[]>("list_watches");
}

export async function createWatch(watch: NewWatch): Promise<Watch> {
  return invoke<Watch>("create_watch", { watch });
}

export type RemoveMode = "everything" | "keep_files" | "files_only";

export async function removeWatch(id: number, mode: RemoveMode): Promise<number> {
  return invoke<number>("remove_watch", { id, mode });
}

export async function setWatchActive(id: number, active: boolean): Promise<void> {
  return invoke<void>("set_watch_active", { id, active });
}

export async function setWatchRating(id: number, rating: number | null): Promise<void> {
  return invoke<void>("set_watch_rating", { id, rating });
}

export async function setWatchListStatus(id: number, listStatus: ListStatus): Promise<void> {
  return invoke<void>("set_watch_list_status", { id, listStatus });
}

export interface WatchPreferences {
  quality: string;
  audio_lang: string | null;
  sub_lang: string | null;
  delete_after_days: number | null;
  notify_on_available: boolean;
  episode_start: number | null;
  episode_end: number | null;
  max_resolution: string | null;
  strip_audio: boolean;
  streaming: boolean;
  delete_after_watched: boolean | null;
}

export async function setWatchPreferences(id: number, prefs: WatchPreferences): Promise<void> {
  return invoke<void>("set_watch_preferences", { id, prefs });
}
