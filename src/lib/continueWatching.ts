import { openPath } from "@tauri-apps/plugin-opener";
import { listWatchEpisodes, type Episode } from "@/lib/episodes";
import { getSettings } from "@/lib/tauri";
import { parseEpisodeNumber, parseSeasonFromTitle } from "@/lib/episodeName";
import type { Watch } from "@/lib/watches";

export function episodeNumberOf(ep: Episode): number | null {
  return ep.episode_number ?? parseEpisodeNumber(ep.name);
}

export function isPlayable(ep: Episode): boolean {
  return ep.status === "available" && !!(ep.item_path ?? ep.save_path);
}

/// Abre o episódio no player escolhido em Config > Reprodução: nativo
/// (página do player do Torii) ou externo (player padrão do Windows).
export async function playEpisode(watch: Watch, episode: Episode, navigate: (to: string) => void) {
  const number = episodeNumberOf(episode);
  const settings = await getSettings().catch(() => ({}) as Record<string, string>);
  const source = episode.item_path ?? episode.save_path;
  if (settings.player_mode === "external" && source) {
    await openPath(source);
  } else if (number != null) {
    navigate(`/watch/${watch.id}/${number}`);
  }
}

/// Episódio pra "continuar assistindo" um anime (todas as temporadas dele
/// na Biblioteca): o 1º baixado e ainda não assistido, da temporada mais
/// antiga pra mais nova. Tudo assistido → o último baixado (rever).
/// `null` = nada baixado ainda.
export async function findContinueEpisode(seasons: Watch[]): Promise<{ watch: Watch; episode: Episode } | null> {
  const ordered = [...seasons].sort(
    (a, b) =>
      (parseSeasonFromTitle(a.title) ?? 0) - (parseSeasonFromTitle(b.title) ?? 0) ||
      (a.anilist_id ?? 0) - (b.anilist_id ?? 0),
  );
  let lastPlayable: { watch: Watch; episode: Episode } | null = null;
  for (const watch of ordered) {
    const episodes = (await listWatchEpisodes(watch.id))
      .filter(isPlayable)
      .sort((a, b) => (episodeNumberOf(a) ?? 0) - (episodeNumberOf(b) ?? 0));
    const unwatched = episodes.find((e) => !e.watched_at);
    if (unwatched) return { watch, episode: unwatched };
    if (episodes.length > 0) lastPlayable = { watch, episode: episodes[episodes.length - 1] };
  }
  return lastPlayable;
}
