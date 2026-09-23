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
  /** "Reduzir resolução" desse anime: "720p" = ligado; null/"original" = desligado.
   * Ligado na Config vale pra todos por cima disso. */
  max_resolution: string | null;
  /** "Remover áudios extras" desse anime — mesma regra. */
  strip_audio: boolean;
  /** Anime (1ª temporada na AniList) dessa temporada — agrupa na Biblioteca.
   * null = ainda não resolvido (cai no agrupamento por título). */
  series_anilist_id: number | null;
  series_title: string | null;
}

/** Chave de agrupamento por anime: franquia da AniList quando resolvida,
 * senão o título sem "Season N". */
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
  /** Atualização mais recente entre as temporadas. */
  updated_at: string;
}

// Temporadas do mesmo anime viram UM card só na grade (título do anime,
// sem "Season N") — a temporada vira aba lá dentro. Agrupa pela franquia na
// AniList (pega temporada sem "Season N" no nome, ex. "Entertainment
// District Arc"), ou pelo título quando ainda não resolvida.
// Representante = temporada mais recente (maior anilist_id ≈ mais nova).
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
}

export async function listWatches(): Promise<Watch[]> {
  return invoke<Watch[]>("list_watches");
}

export async function createWatch(watch: NewWatch): Promise<Watch> {
  return invoke<Watch>("create_watch", { watch });
}

export async function deleteWatch(id: number): Promise<void> {
  return invoke<void>("delete_watch", { id });
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
}

export async function setWatchPreferences(id: number, prefs: WatchPreferences): Promise<void> {
  return invoke<void>("set_watch_preferences", { id, prefs });
}
