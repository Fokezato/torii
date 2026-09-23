import { invoke } from "@tauri-apps/api/core";
import { t } from "@/i18n";

export interface AnimeSummary {
  anilist_id: number;
  title: string;
  cover_url: string | null;
  banner_url: string | null;
  episodes: number | null;
  status: string | null;
  genres: string[];
  score: number | null;
  duration: number | null;
  studio: string | null;
  description: string | null;
  season: string | null;
  season_year: number | null;
  next_airing_at: number | null;
  next_airing_episode: number | null;
  upcoming_episodes: { episode: number; airing_at: number }[];
}

export async function browseSeason(season?: string, year?: number): Promise<AnimeSummary[]> {
  return invoke<AnimeSummary[]>("browse_season", { season, year });
}

export async function browseTrending(): Promise<AnimeSummary[]> {
  return invoke<AnimeSummary[]>("browse_trending");
}

export async function anilistSearch(q: string): Promise<AnimeSummary[]> {
  return invoke<AnimeSummary[]>("anilist_search", { q });
}

export async function getSchedule(ids: number[]): Promise<AnimeSummary[]> {
  return invoke<AnimeSummary[]>("get_schedule", { ids });
}

export async function getAnimeById(id: number): Promise<AnimeSummary | null> {
  const results = await getSchedule([id]);
  return results[0] ?? null;
}

/** Todas as temporadas (TV) do anime, em ordem de lançamento (1ª = raiz). */
export async function getAnimeSeasons(anilistId: number): Promise<AnimeSummary[]> {
  return invoke<AnimeSummary[]>("anime_seasons", { anilistId });
}

/** Nome da aba de temporada: o que sobra do título depois do nome do anime.
 * "Mushoku Tensei: Jobless Reincarnation Season 2 Part 2" → "Temporada 2 Parte 2";
 * "Demon Slayer: Kimetsu no Yaiba Entertainment District Arc" → "Entertainment District Arc";
 * igual ao nome do anime → "Temporada 1". */
export function seasonTabLabel(seasonTitle: string, seriesTitle: string | null): string {
  let rest = seasonTitle;
  if (seriesTitle && seasonTitle.toLowerCase().startsWith(seriesTitle.toLowerCase())) {
    rest = seasonTitle.slice(seriesTitle.length);
  }
  rest = rest.replace(/^[\s:\-–—.]+/, "").trim();
  if (!rest) return t("labels.season", { number: 1 });
  return rest
    .replace(/\bseason\s*(\d+)/i, (_, n) => t("labels.season", { number: Number(n) }))
    .replace(/(\d+)(?:st|nd|rd|th)\s*season/i, (_, n) => t("labels.season", { number: Number(n) }))
    .replace(/\bpart\s*(\d+)/i, (_, n) => t("labels.part", { number: Number(n) }));
}
