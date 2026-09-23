import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Play } from "lucide-react";
import { useTranslation } from "react-i18next";
import { listAvailableEpisodes, type Episode } from "@/lib/episodes";
import type { Watch, WatchGroup } from "@/lib/watches";
import { episodeNumberOf, playEpisode } from "@/lib/continueWatching";
import { formatPlayerTitle, parseEpisodeLabel, parseSeasonFromTitle } from "@/lib/episodeName";
import { mediaFrame, mediaProbe } from "@/lib/player";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";

// Menos que isso de progresso não conta como "começou a ver".
const MIN_STARTED_MS = 5_000;
// Quadro da miniatura do próximo episódio: depois da abertura, em geral.
const NEXT_EPISODE_FRAME_MS = 150_000;

interface ContinueItem {
  key: string;
  watch: Watch;
  episode: Episode;
  /** true = já terminou o anterior, esse ainda não começou. */
  upNext: boolean;
  activityAt: number;
}

/// Um item por anime (todas as temporadas juntas): o episódio parado no
/// meio, ou o próximo baixado depois do último assistido. Do mais recente
/// pro mais antigo.
function buildItems(groups: WatchGroup[], episodes: Episode[]): ContinueItem[] {
  const byWatch = new Map<number, Episode[]>();
  for (const ep of episodes) {
    const list = byWatch.get(ep.watch_id);
    if (list) list.push(ep);
    else byWatch.set(ep.watch_id, [ep]);
  }

  const items: ContinueItem[] = [];
  for (const g of groups) {
    const seasons = [...g.seasons].sort(
      (a, b) =>
        (parseSeasonFromTitle(a.title) ?? 0) - (parseSeasonFromTitle(b.title) ?? 0) ||
        (a.anilist_id ?? 0) - (b.anilist_id ?? 0),
    );
    const ordered = seasons.flatMap((watch) =>
      (byWatch.get(watch.id) ?? [])
        .filter((ep) => episodeNumberOf(ep) != null && (ep.item_path ?? ep.save_path))
        .sort((a, b) => (episodeNumberOf(a) ?? 0) - (episodeNumberOf(b) ?? 0))
        .map((episode) => ({ watch, episode })),
    );

    let lastIndex = -1;
    let lastAt = 0;
    ordered.forEach(({ episode }, i) => {
      const at = episode.watch_progress_at ? new Date(episode.watch_progress_at).getTime() : 0;
      if (at > lastAt) {
        lastAt = at;
        lastIndex = i;
      }
    });
    if (lastIndex < 0) continue;

    const last = ordered[lastIndex];
    if (!last.episode.watched_at && (last.episode.watch_position_ms ?? 0) > MIN_STARTED_MS) {
      items.push({ key: g.key, ...last, upNext: false, activityAt: lastAt });
      continue;
    }
    const next = ordered.slice(lastIndex + 1).find(({ episode }) => !episode.watched_at);
    if (next) items.push({ key: g.key, ...next, upNext: true, activityAt: lastAt });
  }
  return items.sort((a, b) => b.activityAt - a.activityAt);
}

function ContinueCard({ item }: { item: ContinueItem }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { watch, episode, upNext } = item;
  const path = (episode.item_path ?? episode.save_path)!;
  const position = upNext ? 0 : (episode.watch_position_ms ?? 0);

  const { data: probe } = useQuery({
    queryKey: ["media-probe", path],
    queryFn: () => mediaProbe(path),
    staleTime: Infinity,
    retry: false,
  });
  // Arredonda pra reaproveitar o cache de quadros do Rust entre visitas.
  const frameAt = upNext ? NEXT_EPISODE_FRAME_MS : Math.floor(position / 10_000) * 10_000;
  const { data: frame } = useQuery({
    queryKey: ["media-frame", path, frameAt],
    queryFn: () => mediaFrame(path, frameAt),
    staleTime: Infinity,
    retry: false,
  });

  const duration = probe?.duration_ms ?? 0;
  const progress = duration > 0 ? Math.min(1, position / duration) : 0;
  const minutesLeft = duration > 0 ? Math.max(1, Math.round((duration - position) / 60_000)) : null;
  const rawLabel = parseEpisodeLabel(episode.name, episodeNumberOf(episode));
  const { title, episodeLabel } = formatPlayerTitle(watch.title, rawLabel, watch.series_title);
  const image = frame ?? watch.cover_url;

  return (
    <button
      type="button"
      onClick={() => playEpisode(watch, episode, navigate)}
      aria-label={t("library.resume", { title })}
      className="group flex w-72 shrink-0 flex-col gap-2 text-left"
    >
      <div className="relative aspect-video w-full overflow-hidden rounded-lg bg-secondary ring-1 ring-white/5 transition-shadow group-hover:ring-white/20">
        {image ? (
          <img src={image} alt="" className="absolute inset-0 h-full w-full object-cover" />
        ) : (
          <div className="absolute inset-0 bg-gradient-to-br from-secondary to-muted" />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent" />
        <div className="absolute inset-0 flex items-center justify-center bg-black/30 opacity-0 transition-opacity group-hover:opacity-100">
          <span className="flex size-11 items-center justify-center rounded-full border border-white/70 bg-black/50">
            <Play className="size-4 translate-x-0.5 fill-white text-white" />
          </span>
        </div>
        {upNext && (
          <span className="absolute top-2 left-2 rounded-md bg-black/60 px-2 py-0.5 text-[10px] font-bold tracking-wide text-white uppercase backdrop-blur-sm">
            {t("library.upNext")}
          </span>
        )}
        <div className="absolute inset-x-0 bottom-0 h-1 bg-white/20">
          <div className="h-full bg-primary" style={{ width: `${progress * 100}%` }} />
        </div>
      </div>
      <div className="flex min-w-0 flex-col gap-0.5 px-0.5">
        <span className="truncate text-[13px] font-semibold">{title}</span>
        <div className="flex items-center justify-between gap-3 text-xs text-[#8A8F9C]">
          <span className="truncate">{episodeLabel}</span>
          {minutesLeft != null && (
            <span className="shrink-0">
              {upNext ? t("common.minPerEpisode", { count: minutesLeft }) : t("library.timeLeft", { count: minutesLeft })}
            </span>
          )}
        </div>
      </div>
    </button>
  );
}

export function ContinueWatchingRow({ groups }: { groups: WatchGroup[] }) {
  const { t } = useTranslation();
  const { data: episodes } = useQuery({ queryKey: ["available-episodes"], queryFn: listAvailableEpisodes });
  const items = useMemo(() => buildItems(groups, episodes ?? []).slice(0, 12), [groups, episodes]);

  if (items.length === 0) return null;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-lg font-semibold tracking-tight">{t("library.continueWatching")}</h2>
      <ScrollArea className="w-full whitespace-nowrap">
        <div className="flex gap-4 pb-3">
          {items.map((item) => (
            <ContinueCard key={item.key} item={item} />
          ))}
        </div>
        <ScrollBar orientation="horizontal" />
      </ScrollArea>
    </section>
  );
}
