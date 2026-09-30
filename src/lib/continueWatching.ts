import { openPath } from "@tauri-apps/plugin-opener";
import { listWatchEpisodes, type Episode } from "@/lib/episodes";
import { getSettings } from "@/lib/tauri";
import { parseEpisodeNumber, parseSeasonFromTitle } from "@/lib/episodeName";
import type { Watch } from "@/lib/watches";

export function isStreamStartable(watch: Watch | null | undefined, ep: Episode): boolean {
  return !!watch?.streaming && ["ready", "found", "downloading", "error", "deleted"].includes(ep.status);
}

export function episodeNumberOf(ep: Episode): number | null {
  return ep.episode_number ?? parseEpisodeNumber(ep.name);
}

export function isStreamable(ep: Episode): boolean {
  return ep.status === "downloading";
}

export function isPlayable(ep: Episode): boolean {
  return ep.status === "available" && !!ep.item_path;
}

export async function playEpisode(watch: Watch, episode: Episode, navigate: (to: string) => void) {
  const number = episodeNumberOf(episode);
  const settings = await getSettings().catch(() => ({}) as Record<string, string>);
  const source = episode.item_path;
  if (settings.player_mode === "external" && source && isPlayable(episode)) {
    await openPath(source);
  } else if (number != null) {
    navigate(`/watch/${watch.id}/${number}`);
  }
}

export async function findContinueEpisode(seasons: Watch[]): Promise<{ watch: Watch; episode: Episode } | null> {
  const ordered = [...seasons].sort(
    (a, b) =>
      (parseSeasonFromTitle(a.title) ?? 0) - (parseSeasonFromTitle(b.title) ?? 0) ||
      (a.anilist_id ?? 0) - (b.anilist_id ?? 0),
  );
  let lastPlayable: { watch: Watch; episode: Episode } | null = null;
  for (const watch of ordered) {
    const episodes = (await listWatchEpisodes(watch.id))
      .filter((e) => isPlayable(e) || (isStreamable(e) && !!e.watch_progress_at) || isStreamStartable(watch, e))
      .sort((a, b) => (episodeNumberOf(a) ?? 0) - (episodeNumberOf(b) ?? 0));
    const unwatched = episodes.find((e) => !e.watched_at);
    if (unwatched) return { watch, episode: unwatched };
    if (watch.streaming) {
      const pending = (await listWatchEpisodes(watch.id))
        .filter((e) => e.status === "pending" && !e.watched_at && episodeNumberOf(e) != null)
        .sort((a, b) => (episodeNumberOf(a) ?? 0) - (episodeNumberOf(b) ?? 0))
        .find((e) => watch.episode_start == null || (episodeNumberOf(e) ?? 0) >= watch.episode_start);
      if (pending) return { watch, episode: pending };
    }
    if (episodes.length > 0) lastPlayable = { watch, episode: episodes[episodes.length - 1] };
  }
  return lastPlayable;
}
