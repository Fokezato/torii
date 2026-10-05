import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Check,
  CheckCheck,
  EyeOff,
  ChevronLeft,
  Download,
  ExternalLink,
  ListVideo,
  MoreVertical,
  Package,
  Play,
  Plus,
  RefreshCw,
  Settings2,
  Star,
  Trash2,
  Users,
} from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { RemoveWatchDialog } from "@/components/library/RemoveWatchDialog";
import { WatchPreferencesDialog } from "@/components/library/WatchPreferencesDialog";
import {
  getAnimeById,
  getAnimeSeasons,
  getEpisodeMeta,
  seasonTabLabel,
  type AnimeSummary,
  type EpisodeMeta,
} from "@/lib/anilist";
import { mediaFrame, mediaProbe, videoQuality, type ProbeTrack } from "@/lib/player";
import { humanizeTrackLanguage, probeTrackName } from "@/lib/playerLanguage";
import { TranslatedText } from "@/components/shared/TranslatedText";
import { WatchFormModal } from "@/components/anime/WatchFormModal";
import { findContinueEpisode, playEpisode } from "@/lib/continueWatching";
import {
  deleteEpisode,
  downloadMissingEpisodes,
  onDownloadProgress,
  setEpisodeWatched,
  forceCheckEpisode,
  listEpisodeSources,
  listWatchEpisodes,
  packKey,
  switchEpisodeSource,
  type Episode,
} from "@/lib/episodes";
import { notify } from "@/lib/notify";
import { episodeView, TONE_STYLE, type EpisodeGroup, type EpisodeView } from "@/lib/episodeView";
import { isMovie as isMovieWatch, parseEpisodeLabel, parseEpisodeNumber, parseSeasonFromTitle } from "@/lib/episodeName";
import {
  listWatches,
  type RemoveMode,
  seriesKeyOf,
  seriesTitleOf,
  setWatchActive,
  setWatchListStatus,
  setWatchRating,
  type ListStatus,
  type Watch,
} from "@/lib/watches";
import {
  languageLabel,
  listStatusLabel,
  listStatusOptions,
  qualityLabel as qualityName,
  seasonLabel,
  statusLabel,
} from "@/lib/constants";
import { useTranslation } from "react-i18next";

export default function LibraryDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [prefsOpen, setPrefsOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const queryClient = useQueryClient();

  const { data: watches, isLoading: watchesLoading } = useQuery({
    queryKey: ["watches"],
    queryFn: listWatches,
  });
  const entryWatch = watches?.find((w) => w.id === Number(id));

  const franchiseRoot = entryWatch?.series_anilist_id ?? entryWatch?.anilist_id ?? null;
  const { data: franchise = [] } = useQuery({
    queryKey: ["anime-seasons", franchiseRoot],
    queryFn: () => getAnimeSeasons(franchiseRoot!),
    enabled: franchiseRoot != null,
    staleTime: 30 * 60_000,
  });
  const franchiseIndex = (w: Watch) => {
    const i = franchise.findIndex((f) => f.anilist_id === w.anilist_id);
    return i >= 0 ? i : 1000 + (parseSeasonFromTitle(w.title) ?? 0);
  };

  const seasons: Watch[] = entryWatch
    ? (watches ?? [])
        .filter((w) => seriesKeyOf(w) === seriesKeyOf(entryWatch))
        .sort((a, b) => franchiseIndex(a) - franchiseIndex(b))
    : [];
  const missingSeasons = franchise.filter((f) => !seasons.some((s) => s.anilist_id === f.anilist_id));
  const [addSeason, setAddSeason] = useState<AnimeSummary | null>(null);
  const [addSeasonMenuOpen, setAddSeasonMenuOpen] = useState(false);

  useEffect(() => {
    setSelectedId(Number(id));
  }, [id]);

  const watch = seasons.find((w) => w.id === selectedId) ?? entryWatch;

  const { data: anime } = useQuery({
    queryKey: ["anime-by-id", watch?.anilist_id],
    queryFn: () => getAnimeById(watch!.anilist_id!),
    enabled: watch?.anilist_id != null,
  });
  const { data: episodeMetaList } = useQuery({
    queryKey: ["episode-meta", watch?.anilist_id],
    queryFn: () => getEpisodeMeta(watch!.anilist_id!),
    enabled: watch?.anilist_id != null,
    staleTime: 6 * 3600_000,
    retry: 1,
  });
  const episodeMeta = new Map((episodeMetaList ?? []).map((m) => [m.number, m]));

  const upcomingByEpisode = new Map((anime?.upcoming_episodes ?? []).map((u) => [u.episode, u.airing_at]));

  const { data: episodeList = [] } = useQuery({
    queryKey: ["watch-episodes", watch?.id],
    queryFn: () => listWatchEpisodes(watch!.id),
    enabled: watch != null,
    refetchInterval: 10_000,
  });

  const [downloadPct, setDownloadPct] = useState<Record<number, number>>({});
  useEffect(() => {
    const unlisten = onDownloadProgress((batch) => {
      setDownloadPct((prev) => {
        const next = { ...prev };
        for (const p of batch) next[p.episode_id] = p.total_bytes > 0 ? (p.progress_bytes / p.total_bytes) * 100 : 0;
        return next;
      });
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);
  const [episodeFilter, setEpisodeFilter] = useState<"all" | "unwatched" | EpisodeGroup>("all");

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ["watches"] });

  const patchActive = useMutation({
    mutationFn: (active: boolean) => setWatchActive(watch!.id, active),
    onSuccess: invalidate,
  });
  const patchStatus = useMutation({
    mutationFn: (status: ListStatus) => setWatchListStatus(watch!.id, status),
    onSuccess: invalidate,
  });
  const patchRating = useMutation({
    mutationFn: (rating: number) => setWatchRating(watch!.id, rating),
    onSuccess: invalidate,
  });
  const [removeOpen, setRemoveOpen] = useState(false);
  const downloadMissing = useMutation({
    mutationFn: () => downloadMissingEpisodes(watch!.id),
    onSuccess: () => {
      setTimeout(() => queryClient.invalidateQueries({ queryKey: ["watch-episodes"] }), 4000);
    },
  });
  function onRemoved(mode: RemoveMode) {
    invalidate();
    queryClient.invalidateQueries({ queryKey: ["watch-episodes"] });
    if (mode !== "files_only") navigate("/library");
  }

  if (watchesLoading) {
    return <p className="text-sm text-muted-foreground">{t("common.loading")}</p>;
  }

  if (!watch) {
    return (
      <div className="space-y-4">
        <p className="text-sm text-muted-foreground">{t("detail.notFound")}</p>
        <button
          type="button"
          onClick={() => navigate("/library")}
          className="text-sm font-semibold text-primary"
        >
          {t("detail.backToLibrary")}
        </button>
      </div>
    );
  }

  const banner = anime?.banner_url ?? anime?.cover_url ?? watch.cover_url;
  const episodes = watch.episodes ?? anime?.episodes;
  const isMovie = isMovieWatch(watch);
  const movieEpisode = isMovie
    ? episodeList.slice().sort((a, b) => MOVIE_RANK.indexOf(a.status) - MOVIE_RANK.indexOf(b.status))[0]
    : undefined;
  const seriesTitle = seriesTitleOf(watch);
  const tabLabel = (w: Watch) =>
    seasonTabLabel(franchise.find((f) => f.anilist_id === w.anilist_id)?.title ?? w.title, seriesTitle);
  const downloadedCount = episodeList.filter((e) => e.status === "available").length;
  const baseTitle = seriesTitle;
  const latestAvailable = episodeList
    .filter((e) => e.status === "available")
    .slice()
    .sort((a, b) => (b.episode_number ?? parseEpisodeNumber(b.name) ?? 0) - (a.episode_number ?? parseEpisodeNumber(a.name) ?? 0))[0];
  const downloadingAny = episodeList.some((e) => e.status === "downloading" || e.status === "found");

  function selectSeason(seasonWatch: Watch) {
    setSelectedId(seasonWatch.id);
    navigate(`/library/${seasonWatch.id}`, { replace: true });
  }

  const metaLine = [
    anime?.season_year && anime.season
      ? `${anime.season_year} · ${seasonLabel(anime.season)}`
      : null,
    anime?.studio,
    anime?.duration
      ? isMovie
        ? movieLength(anime.duration * 60_000)
        : t("common.minPerEpisode", { count: anime.duration })
      : null,
    episodes && !isMovie ? t("common.episodeCount", { count: episodes }) : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="flex flex-col gap-6">
      <button
        type="button"
        onClick={() => navigate("/library")}
        className="flex w-fit items-center gap-1.5 text-xs font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
      >
        <ChevronLeft className="size-3.5" />
        {t("nav.library")}
      </button>

      <div className="relative h-[210px] shrink-0 overflow-hidden rounded-[18px] bg-secondary">
        {banner && <img src={banner} alt="" className="absolute inset-0 h-full w-full object-cover" />}
        <div className="absolute inset-0 bg-gradient-to-t from-background via-background/20 to-transparent" />
        {watch.status && (
          <span className="absolute bottom-4 left-5 rounded-md bg-accent2 px-2.5 py-1 text-[11px] font-bold tracking-wide text-background uppercase">
            {statusLabel(watch.status)}
            {episodes && !isMovie ? ` · ${episodes} eps` : ""}
          </span>
        )}
      </div>

      <div className="flex flex-col gap-3.5">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-[28px] font-bold">{baseTitle}</h1>
            <Select value={watch.list_status} onValueChange={(v) => patchStatus.mutate(v as ListStatus)}>
              <SelectTrigger
                size="sm"
                style={{ backgroundColor: "var(--primary)", color: "var(--primary-foreground)" }}
                className="rounded-md border-0 px-2 text-[11px] font-bold tracking-wide uppercase [&_svg]:text-primary-foreground"
              >
                <SelectValue>{listStatusLabel(watch.list_status)}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {listStatusOptions().map((o) => (
                  <SelectItem key={o.value} value={o.value}>
                    {o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex shrink-0 items-center gap-2.5">
            {latestAvailable || watch.streaming ? (
              <button
                type="button"
                onClick={async () => {
                  const target = await findContinueEpisode([watch]).catch(() => null);
                  if (target) await playEpisode(target.watch, target.episode, navigate);
                }}
                className="flex items-center gap-2 rounded-[10px] bg-primary px-4.5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90"
              >
                <Play className="size-3.5" fill="currentColor" />
                {t("detail.play")}
              </button>
            ) : (
              <button
                type="button"
                disabled={downloadingAny || downloadMissing.isPending}
                onClick={() => downloadMissing.mutate()}
                className="flex items-center gap-2 rounded-[10px] bg-primary px-4.5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                <Download className="size-3.5" />
                {downloadingAny || downloadMissing.isPending ? t("detail.downloadingAll") : t("detail.downloadAll")}
              </button>
            )}
            <button
              type="button"
              onClick={() => setPrefsOpen(true)}
              className="flex items-center gap-2 rounded-[10px] border border-[#33374A] px-3.5 py-2.5 text-[13px] font-semibold text-foreground transition-colors hover:bg-white/5"
            >
              <Settings2 className="size-3.5" />
              {t("detail.preferences")}
            </button>
            <button
              type="button"
              onClick={() => setRemoveOpen(true)}
              className="flex items-center gap-2 rounded-[10px] border border-[#3A2A30] px-3.5 py-2.5 text-[13px] font-semibold text-[#B5576B] transition-colors hover:bg-[#B5576B]/10"
            >
              <Trash2 className="size-3.5" />
              {t("detail.remove")}
            </button>
          </div>
        </div>

        {metaLine && <p className="text-[12.5px] text-[#8A8F9C]">{metaLine}</p>}

        <div className="flex flex-wrap items-center gap-x-7 gap-y-3">
          <div className="flex items-center gap-1.5">
            <span className="mr-1 text-xs text-[#B5B9C4]">{t("detail.yourRating")}</span>
            {[1, 2, 3, 4, 5].map((n) => (
              <button key={n} type="button" onClick={() => patchRating.mutate(n)}>
                <Star
                  className={
                    n <= (watch.rating ?? 0)
                      ? "size-4 fill-yellow-400 text-yellow-400"
                      : "size-4 text-[#4E5361]"
                  }
                />
              </button>
            ))}
          </div>

          {anime?.score != null && (
            <div className="flex items-center gap-1.5 text-xs text-[#B5B9C4]">
              {t("detail.audienceScore")}
              <Star className="size-[13px] fill-accent2 text-accent2" />
              <span className="text-sm font-bold text-foreground">{(anime.score / 10).toFixed(1)}</span>
            </div>
          )}

          <label className="flex items-center gap-2.5 text-xs text-[#B5B9C4]">
            {t("detail.checkNewEpisodes")}
            <Switch checked={watch.active} onCheckedChange={(v) => patchActive.mutate(v)} />
          </label>
        </div>

        {anime?.genres && anime.genres.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            {anime.genres.map((g) => (
              <span
                key={g}
                className="rounded-full border border-white/10 bg-white/[0.06] px-2.5 py-1 text-[11px] font-semibold text-[#B5B9C4]"
              >
                {g}
              </span>
            ))}
          </div>
        )}

        {anime?.description && (
          <Synopsis text={anime.description} />
        )}
      </div>

      <div className="flex flex-col gap-3.5">
        <div className="flex flex-wrap items-center gap-2.5">
          <h2 className="text-lg font-bold">{isMovie ? t("detail.movie") : t("detail.episodes")}</h2>
          <div className="flex flex-wrap items-center gap-2">
            {seasons.map((s) => (
              <button
                key={s.id}
                type="button"
                aria-pressed={s.id === watch.id}
                onClick={() => selectSeason(s)}
                className={`rounded-full px-3 py-1 text-[11px] font-semibold transition-colors ${
                  s.id === watch.id
                    ? "bg-primary text-primary-foreground"
                    : "bg-secondary text-[#B5B9C4] hover:text-foreground"
                }`}
              >
                {tabLabel(s)}
              </button>
            ))}
            {missingSeasons.length > 0 && (
              <Popover open={addSeasonMenuOpen} onOpenChange={setAddSeasonMenuOpen}>
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    className="flex items-center gap-1 rounded-full border border-dashed border-[#33374A] px-3 py-1 text-[11px] font-semibold text-[#B5B9C4] transition-colors hover:border-primary hover:text-foreground"
                  >
                    <Plus className="size-3" />
                    {t("detail.anotherSeason")}
                  </button>
                </PopoverTrigger>
                <PopoverContent align="start" className="w-72 p-1.5">
                  <span className="block px-2.5 pt-1 pb-1.5 text-[10px] font-bold tracking-wide text-[#6C7180] uppercase">
                    {t("detail.downloadAnotherSeason")}
                  </span>
                  {missingSeasons.map((f) => (
                    <button
                      key={f.anilist_id}
                      type="button"
                      onClick={() => {
                        setAddSeasonMenuOpen(false);
                        setAddSeason(f);
                      }}
                      className="flex w-full items-center justify-between gap-3 rounded-md px-2.5 py-2 text-left text-xs transition-colors hover:bg-white/5"
                    >
                      <span className="min-w-0 truncate font-semibold">{seasonTabLabel(f.title, seriesTitle)}</span>
                      <span className="shrink-0 text-[11px] text-[#6C7180]">
                        {[f.season_year, f.episodes ? `${f.episodes} eps` : null].filter(Boolean).join(" · ")}
                      </span>
                    </button>
                  ))}
                </PopoverContent>
              </Popover>
            )}
          </div>
          {!isMovie && (
            <span className="text-xs text-[#6C7180]">
              {episodes
                ? t("detail.downloadedOf", { count: downloadedCount, total: episodes })
                : t("detail.downloaded", { count: downloadedCount })}
            </span>
          )}
        </div>
        {isMovie && movieEpisode ? (
          <MoviePanel
            episode={movieEpisode}
            watch={watch}
            image={anime?.banner_url ?? watch.cover_url}
            durationMs={anime?.duration ? anime.duration * 60_000 : undefined}
            downloadPct={downloadPct[movieEpisode.id]}
          />
        ) : episodeList.length === 0 ? (
          <div className="rounded-xl border border-dashed border-[#23262F] p-6 text-center text-xs text-[#6C7180]">
            {t("detail.noEpisodes")}
          </div>
        ) : (
          <EpisodeList
            episodes={episodeList}
            watch={watch}
            upcoming={upcomingByEpisode}
            durationMs={anime?.duration ? anime.duration * 60_000 : undefined}
            downloadPct={downloadPct}
            meta={episodeMeta}
            filter={episodeFilter}
            onFilter={setEpisodeFilter}
          />
        )}
      </div>

      <WatchPreferencesDialog watch={prefsOpen ? watch : null} onOpenChange={setPrefsOpen} />
      <RemoveWatchDialog watch={removeOpen ? watch : null} onOpenChange={setRemoveOpen} onDone={onRemoved} />
      <WatchFormModal anime={addSeason} onOpenChange={(open) => !open && setAddSeason(null)} />
    </div>
  );
}

function PackBanner({ episodes }: { episodes: Episode[] }) {
  const { t } = useTranslation();
  const first = episodes[0];
  const { data: sources } = useQuery({
    queryKey: ["episode-sources", first.id],
    queryFn: () => listEpisodeSources(first.id),
    staleTime: Infinity,
  });
  const title = sources?.find((s) => s.source_item_id === first.source_item_id)?.title ?? "";
  const numbers = episodes.map((e) => e.episode_number).filter((n): n is number => n != null).sort((a, b) => a - b);
  const range = numbers.length ? `${numbers[0]}–${numbers[numbers.length - 1]}` : "";
  const ready = episodes.filter((e) => e.status === "available").length;
  return (
    <div className="flex items-center gap-3 rounded-[10px] border border-accent2/30 bg-accent2/[0.06] px-4 py-3">
      <Package className="size-4 shrink-0 text-accent2" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-[12.5px] font-semibold">{t("detail.packBanner", { range })}</span>
        <span className="truncate text-[11px] text-[#8A8F9C]" title={title}>
          {title}
        </span>
      </div>
      <span className="shrink-0 text-[11px] font-semibold text-accent2">
        {t("detail.packReady", { ready, total: episodes.length })}
      </span>
    </div>
  );
}

/// One banner per season pack in use by this season's episodes.
function PackBanners({ episodes }: { episodes: Episode[] }) {
  const groups = new Map<string, Episode[]>();
  for (const e of episodes) {
    if (e.status === "deleted") continue;
    const key = packKey(e);
    if (!key) continue;
    groups.set(key, [...(groups.get(key) ?? []), e]);
  }
  return (
    <>
      {[...groups.entries()].map(([key, group]) => (
        <PackBanner key={key} episodes={group} />
      ))}
    </>
  );
}

function useEpisodeMedia(episode: Episode | undefined) {
  const localPath = episode?.status === "available" ? episode.item_path : null;
  const { data: probe } = useQuery({
    queryKey: ["media-probe", localPath],
    queryFn: () => mediaProbe(localPath!),
    enabled: localPath != null,
    staleTime: Infinity,
    retry: false,
  });
  // Same 30% point as the player's episode panel, so the frame cache is shared.
  const frameAt = probe && probe.duration_ms > 0 ? Math.round(probe.duration_ms * 0.3) : null;
  const { data: frame } = useQuery({
    queryKey: ["media-frame", localPath, frameAt],
    queryFn: () => mediaFrame(localPath!, frameAt!),
    enabled: localPath != null && frameAt != null,
    staleTime: Infinity,
    retry: false,
  });
  return { probe, frame };
}

function movieLength(ms: number): string {
  const total = Math.round(ms / 60_000);
  const h = Math.floor(total / 60);
  const m = total % 60;
  return h > 0 ? `${h}h ${String(m).padStart(2, "0")}min` : `${m}min`;
}

function Synopsis({ text }: { text: string }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const long = text.length > 480;
  return (
    <div className="flex max-w-[1150px] flex-col items-start gap-1">
      <p className={`text-[13.5px] leading-relaxed text-[#C7CAD3] ${long && !expanded ? "line-clamp-4" : ""}`}>{text}</p>
      {long && (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="text-[12px] font-semibold text-primary hover:underline"
        >
          {expanded ? t("detail.synopsisLess") : t("detail.synopsisMore")}
        </button>
      )}
    </div>
  );
}

function MoviePanel({
  episode,
  watch,
  image,
  durationMs,
  downloadPct,
}: {
  episode: Episode;
  watch: Watch;
  image: string | null;
  durationMs?: number;
  downloadPct?: number;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const view = episodeView(episode, watch, { durationMs, downloadPct, now: Date.now() });
  const style = TONE_STYLE[view.tone];
  const { probe, frame } = useEpisodeMedia(episode);
  const quality = episode.status !== "pending" ? videoQuality(probe, episode.name) : null;
  const length = probe?.duration_ms || durationMs;
  const position = episode.watch_position_ms ?? 0;
  const resume = view.tone === "progress";
  const play = () => navigate(`/watch/${watch.id}/${episode.episode_number ?? 1}`);
  const picture = frame ?? image;
  const languages = (tracks: ProbeTrack[]) => [
    ...new Set(tracks.map((tr) => humanizeTrackLanguage(probeTrackName(tr.language, tr.description))).filter(Boolean)),
  ];
  const audio = probe ? languages(probe.audio) : [];
  const subtitles = probe ? languages(probe.subtitles) : [];

  return (
    <div className="relative flex flex-col gap-5 rounded-xl border border-[#1E212A] bg-[#15171D] p-4 md:flex-row">
      <button
        type="button"
        disabled={!view.playable}
        onClick={play}
        aria-label={t("detail.watchMovie")}
        className="group relative aspect-video w-full shrink-0 overflow-hidden rounded-lg bg-[#1E212A] md:w-[440px]"
      >
        {picture && <img src={picture} alt="" className="absolute inset-0 h-full w-full object-cover" />}
        {view.playable && (
          <span className="absolute inset-0 flex items-center justify-center bg-black/25 transition-colors group-hover:bg-black/45">
            <span className="flex size-14 items-center justify-center rounded-full bg-primary/90 text-primary-foreground transition-transform group-hover:scale-105">
              <Play className="ml-0.5 size-6" fill="currentColor" />
            </span>
          </span>
        )}
        {view.watchProgress != null && (
          <span className="absolute inset-x-0 bottom-0 h-1 bg-black/50">
            <span className="block h-full bg-primary" style={{ width: `${view.watchProgress * 100}%` }} />
          </span>
        )}
      </button>

      <div className="flex min-w-0 flex-1 flex-col justify-center gap-3 py-1">
        <div className="flex flex-wrap items-center gap-2">
          <span
            className="rounded-[5px] px-[7px] py-[3px] text-[10px] font-bold tracking-wide uppercase"
            style={{ color: style.fg, background: style.bg, border: style.border ? `1px solid ${style.border}` : undefined }}
          >
            {view.label}
          </span>
          {quality && (
            <span className="rounded-[5px] border border-[#2A2E39] px-[7px] py-[2px] text-[10px] font-bold text-[#B5B9C4]">
              {quality}
            </span>
          )}
          {length ? <span className="text-[12px] text-[#8A8F9C]">{movieLength(length)}</span> : null}
        </div>

        {view.detail && (
          <p className={`text-[12.5px] ${view.tone === "error" ? "text-destructive" : "text-[#8A8F9C]"}`}>{view.detail}</p>
        )}
        {episode.status === "downloading" && (
          <div className="h-1.5 w-full max-w-[420px] overflow-hidden rounded-full bg-[#22252E]">
            <div className="h-full bg-primary transition-[width]" style={{ width: `${downloadPct ?? 0}%` }} />
          </div>
        )}
        {audio.length > 0 && (
          <p className="text-[12.5px] text-[#B5B9C4]">
            <span className="text-[#6C7180]">{t("detail.audio")}:</span> {audio.join(", ")}
          </p>
        )}
        {subtitles.length > 0 && (
          <p className="text-[12.5px] text-[#B5B9C4]">
            <span className="text-[#6C7180]">{t("detail.subtitle")}:</span> {subtitles.join(", ")}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2 pt-2">
          {view.playable && (
            <button
              type="button"
              onClick={play}
              className="flex items-center gap-2 rounded-[10px] bg-primary px-4.5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90"
            >
              <Play className="size-3.5" fill="currentColor" />
              {resume ? t("detail.resumeAt", { time: movieLength(position) }) : t("detail.watchMovie")}
            </button>
          )}
        </div>
      </div>
      <div className="absolute top-3 right-3">
        <EpisodeActionsMenu episode={episode} watch={watch} outOfRange={false} />
      </div>
    </div>
  );
}

const MOVIE_RANK = ["available", "downloading", "ready", "found", "error", "deleted", "pending"];

function EpisodeList({
  episodes,
  watch,
  upcoming,
  durationMs,
  downloadPct,
  meta,
  filter,
  onFilter,
}: {
  episodes: Episode[];
  watch: Watch;
  upcoming: Map<number, number>;
  durationMs?: number;
  downloadPct: Record<number, number>;
  meta: Map<number, EpisodeMeta>;
  filter: "all" | "unwatched" | EpisodeGroup;
  onFilter: (f: "all" | "unwatched" | EpisodeGroup) => void;
}) {
  const { t } = useTranslation();
  const now = Date.now();
  const rows = episodes
    .slice()
    .sort((a, b) => (a.episode_number ?? parseEpisodeNumber(a.name) ?? 0) - (b.episode_number ?? parseEpisodeNumber(b.name) ?? 0))
    .map((ep) => ({
      ep,
      view: episodeView(ep, watch, {
        airingAt: upcoming.get(ep.episode_number ?? -1),
        durationMs,
        downloadPct: downloadPct[ep.id],
        now,
      }),
    }));
  const next = rows.find((r) => r.view.playable && r.view.group !== "watched");
  const count = (g: EpisodeGroup) => rows.filter((r) => r.view.group === g).length;
  const downloading = rows.filter((r) => r.ep.status === "downloading").length;
  const visible = rows.filter(({ view }) =>
    filter === "all" ? true : filter === "unwatched" ? view.group !== "watched" && view.tone !== "muted" : view.group === filter,
  );
  const chips: { id: "all" | "unwatched" | EpisodeGroup; label: string }[] = [
    { id: "all", label: t("epView.filterAll", { count: rows.length }) },
    { id: "unwatched", label: t("epView.filterUnwatched") },
    { id: "watched", label: t("epView.summaryWatched", { count: count("watched") }) },
    { id: "ready", label: t("epView.summaryReady", { count: count("ready") }) },
    { id: "waiting", label: t("epView.summaryWaiting", { count: count("waiting") }) },
  ];

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {chips.map((c) => (
          <button
            key={c.id}
            type="button"
            aria-pressed={filter === c.id}
            onClick={() => onFilter(c.id)}
            className={`rounded-full px-3 py-1 text-[11.5px] font-semibold transition-colors ${
              filter === c.id
                ? "bg-primary text-primary-foreground"
                : "border border-[#23262F] text-[#B5B9C4] hover:text-foreground"
            }`}
          >
            {c.label}
          </button>
        ))}
        {downloading > 0 && (
          <span className="ml-auto text-[11px] text-[#8A8F9C]">{t("epView.summaryDownloading", { count: downloading })}</span>
        )}
      </div>
      <PackBanners episodes={episodes} />
      {visible.map(({ ep, view }) => (
        <EpisodeItem
          key={ep.id}
          episode={ep}
          watch={watch}
          view={view}
          meta={ep.episode_number != null ? meta.get(ep.episode_number) : undefined}
          isNext={next?.ep.id === ep.id}
        />
      ))}
      {visible.length === 0 && (
        <p className="py-6 text-center text-xs text-[#6C7180]">{t("epView.filterEmpty")}</p>
      )}
    </div>
  );
}

function EpisodeItem({
  episode,
  watch,
  view,
  meta,
  isNext,
}: {
  episode: Episode;
  watch: Watch;
  view: EpisodeView;
  meta?: EpisodeMeta;
  isNext: boolean;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const n = episode.episode_number;
  const style = TONE_STYLE[view.tone];
  const play = () => n != null && navigate(`/watch/${watch.id}/${n}`);
  const dim = view.tone === "muted" || view.tone === "removed";
  const { probe, frame } = useEpisodeMedia(episode);

  const image = frame ?? meta?.thumbnail ?? watch.cover_url;
  const coverOnly = !frame && !meta?.thumbnail;
  const quality = episode.status !== "pending" ? videoQuality(probe, episode.name) : null;
  const label = parseEpisodeLabel(episode.name, episode.episode_number);

  return (
    <div
      className={`relative flex items-center gap-3.5 overflow-hidden rounded-[10px] border bg-[#15171D] p-2.5 pr-3 ${
        isNext ? "border-primary/60" : "border-[#1E212A]"
      }`}
    >
      <button
        type="button"
        disabled={!view.playable}
        onClick={play}
        aria-label={t("epView.watch")}
        className={`group relative aspect-video w-36 shrink-0 overflow-hidden rounded-[7px] bg-[#1E212A] ${dim ? "opacity-50" : ""}`}
      >
        {image && (
          <img
            src={image}
            alt=""
            loading="lazy"
            className={`absolute inset-0 h-full w-full object-cover ${coverOnly ? "scale-110 blur-[2px] brightness-75" : ""}`}
          />
        )}
        {view.playable && (
          <span className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
            <Play className="size-5 text-white" fill="currentColor" />
          </span>
        )}
        {view.watchProgress != null && (
          <span className="absolute inset-x-0 bottom-0 h-[3px] bg-black/50">
            <span className="block h-full bg-primary" style={{ width: `${view.watchProgress * 100}%` }} />
          </span>
        )}
      </button>
      <div className={`flex min-w-0 flex-1 flex-col gap-0.5 ${dim ? "opacity-60" : ""}`}>
        <span className="flex min-w-0 items-center gap-2" title={episode.name ?? undefined}>
          <span className="shrink-0 text-[10.5px] font-semibold tracking-wide text-[#8A8F9C] uppercase">{label}</span>
          <span
            className="shrink-0 rounded-[5px] px-[6px] py-[2px] text-[9px] font-bold tracking-wide uppercase"
            style={{ color: style.fg, background: style.bg, border: style.border ? `1px solid ${style.border}` : undefined }}
          >
            {view.label}
          </span>
          {quality && (
            <span className="shrink-0 rounded-[4px] border border-[#2A2E39] px-1.5 py-px text-[9px] font-bold text-[#B5B9C4]">
              {quality}
            </span>
          )}
          {packKey(episode) && episode.status !== "deleted" && (
            <span className="flex shrink-0 items-center gap-1 rounded-[4px] bg-accent2/15 px-1.5 py-px text-[9px] font-bold tracking-wide text-accent2 uppercase">
              <Package className="size-2.5" />
              {t("detail.packBadge")}
            </span>
          )}
        </span>
        {meta?.title && (
          <span className="truncate text-[13px] font-semibold" title={meta.title}>
            {meta.title}
          </span>
        )}
        {meta?.synopsis && (
          <TranslatedText text={meta.synopsis} className="line-clamp-2 text-[11.5px] leading-snug text-[#8A8F9C]" />
        )}
        {view.detail && (
          <span className={`truncate text-[11px] ${view.tone === "error" ? "text-destructive" : "text-[#6C7180]"}`} title={view.detail}>
            {view.detail}
          </span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {view.playable && isNext && (
          <button
            type="button"
            onClick={play}
            className="flex items-center gap-1.5 rounded-[8px] bg-primary px-2.5 py-1 text-[11px] font-semibold text-primary-foreground transition-opacity hover:opacity-90"
          >
            <Play className="size-3" fill="currentColor" />
            {t("epView.watch")}
          </button>
        )}
        {view.tone !== "airing" && (
          <EpisodeActionsMenu episode={episode} watch={watch} outOfRange={view.tone === "muted"} />
        )}
      </div>
    </div>
  );
}

function EpisodeActionsMenu({
  episode,
  watch,
  outOfRange,
}: {
  episode: Episode;
  watch: Watch;
  outOfRange: boolean;
}) {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const queryClient = useQueryClient();
  const deletable = ["available", "downloading", "found", "ready", "error"].includes(episode.status);

  const toggleWatched = useMutation({
    mutationFn: () => setEpisodeWatched(episode.id, !episode.watched_at),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["watch-episodes"] }),
  });

  const remove = useMutation({
    mutationFn: () => deleteEpisode(episode.id),
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["watch-episodes"] });
      queryClient.invalidateQueries({ queryKey: ["recent-episodes"] });
    },
  });

  const forceCheck = useMutation({
    mutationFn: () => forceCheckEpisode(episode.id),
    onSuccess: (found) => {
      queryClient.invalidateQueries({ queryKey: ["watch-episodes"] });
      if (!found) {
        notify(
          t("detail.forceCheck"),
          t("detail.forceCheckNothing", { episode: parseEpisodeLabel(episode.name, episode.episode_number) }),
          "info",
        );
      }
    },
  });

  return (
    <>
      <Popover open={menuOpen} onOpenChange={setMenuOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            aria-label={t("detail.moreOptions")}
            title={t("detail.moreOptions")}
            className="flex size-7 shrink-0 items-center justify-center rounded-[8px] border border-[#262A35] text-[#B5B9C4] hover:bg-secondary"
          >
            <MoreVertical className="size-3.5" />
          </button>
        </PopoverTrigger>
        <PopoverContent align="end" className="w-48 p-1.5">
          <button
            type="button"
            onClick={() => {
              setMenuOpen(false);
              setSourceOpen(true);
            }}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs transition-colors hover:bg-white/5"
          >
            <ListVideo className="size-3.5 text-[#6C7180]" />
            {t("detail.source")}
          </button>
          <button
            type="button"
            disabled={forceCheck.isPending}
            onClick={() => {
              setMenuOpen(false);
              forceCheck.mutate();
            }}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs transition-colors hover:bg-white/5 disabled:opacity-40"
          >
            <RefreshCw className={`size-3.5 text-[#6C7180] ${forceCheck.isPending ? "animate-spin" : ""}`} />
            {episode.status === "deleted"
              ? t("detail.downloadAgain")
              : outOfRange
                ? t("detail.downloadNow")
                : t("detail.forceCheck")}
          </button>
          <button
            type="button"
            disabled={toggleWatched.isPending}
            onClick={() => {
              setMenuOpen(false);
              toggleWatched.mutate();
            }}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs transition-colors hover:bg-white/5 disabled:opacity-40"
          >
            {episode.watched_at ? (
              <EyeOff className="size-3.5 text-[#6C7180]" />
            ) : (
              <CheckCheck className="size-3.5 text-[#6C7180]" />
            )}
            {episode.watched_at ? t("epView.markUnwatched") : t("epView.markWatched")}
          </button>
          {deletable && (
            <button
              type="button"
              disabled={remove.isPending}
              onClick={() => {
                setMenuOpen(false);
                setConfirmDelete(true);
              }}
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-xs text-[#E5484D] transition-colors hover:bg-[#E5484D]/10 disabled:opacity-40"
            >
              <Trash2 className="size-3.5" />
              {t("detail.deleteEpisode")}
            </button>
          )}
        </PopoverContent>
      </Popover>
      <SourceDialog episode={episode} watch={watch} open={sourceOpen} onOpenChange={setSourceOpen} />
      <AlertDialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("detail.deleteEpisodeTitle", { episode: parseEpisodeLabel(episode.name, episode.episode_number) })}
            </AlertDialogTitle>
            <AlertDialogDescription>{t("detail.deleteEpisodeText")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => remove.mutate()}>
              {t("detail.deleteEpisodeConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function SourceDialog({
  episode,
  watch,
  open,
  onOpenChange,
}: {
  episode: Episode;
  watch: Watch;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const qualityLabel = qualityName(watch.quality);
  const audioLabel = watch.audio_lang
    ? watch.audio_lang.split(",").map(languageLabel).join(", ")
    : t("common.any");
  const subLabel = watch.sub_lang ? watch.sub_lang.split(",").map(languageLabel).join(", ") : t("common.any");

  const { data: sources = [], isLoading } = useQuery({
    queryKey: ["episode-sources", episode.id],
    queryFn: () => listEpisodeSources(episode.id),
    enabled: open,
  });

  const switchSource = useMutation({
    mutationFn: (sourceItemId: string) => switchEpisodeSource(episode.id, sourceItemId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["watch-episodes"] });
      queryClient.invalidateQueries({ queryKey: ["episode-sources", episode.id] });
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "480px", width: "92vw" }}
        className="grid-cols-[minmax(0,1fr)] gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
      >
        <div className="flex min-w-0 flex-col gap-2 border-b border-[#1E212A] px-[22px] py-[18px]">
          <h2 className="line-clamp-2 text-base font-bold [overflow-wrap:anywhere]">
            {t("detail.source")} · {parseEpisodeLabel(episode.name, episode.episode_number)}
          </h2>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-[#6C7180]">
            <span>
              {t("detail.quality")}: <span className="text-[#B5B9C4]">{qualityLabel}</span>
            </span>
            <span>
              {t("detail.audio")}: <span className="text-[#B5B9C4]">{audioLabel}</span>
            </span>
            <span>
              {t("detail.subtitle")}: <span className="text-[#B5B9C4]">{subLabel}</span>
            </span>
          </div>
        </div>

        <div className="flex max-h-[60vh] flex-col gap-2 overflow-y-auto p-[18px]">
          {isLoading ? (
            <p className="px-1 py-2 text-xs text-[#6C7180]">{t("common.loading")}</p>
          ) : sources.length === 0 ? (
            <p className="px-1 py-2 text-xs text-[#6C7180]">{t("detail.noSourceInfo")}</p>
          ) : (
            sources.map((s) => (
              <div
                key={s.source_item_id}
                className={`flex flex-col gap-2 rounded-[10px] border px-3.5 py-3 ${
                  s.is_active ? "border-primary/50 bg-primary/[0.06]" : "border-[#1E212A] bg-[#1B1E27]"
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <p className="min-w-0 flex-1 text-[12px] leading-snug [overflow-wrap:anywhere]">{s.title}</p>
                  {!!s.is_active && (
                    <span className="flex shrink-0 items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[10px] font-bold text-primary">
                      <Check className="size-3" />
                      {t("detail.activeSource")}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-3 text-[11px] text-[#6C7180]">
                  <span className="flex items-center gap-1">
                    <Users className="size-3" />
                    {s.seeders ?? 0} seed · {s.leechers ?? 0} leech
                  </span>
                  {s.size && <span>{s.size}</span>}
                </div>
                <div className="flex items-center gap-2 pt-1">
                  <button
                    type="button"
                    onClick={() => openUrl(`https://nyaa.si/view/${s.source_item_id}`)}
                    className="flex items-center gap-1.5 rounded-[8px] border border-[#262A35] px-2.5 py-1.5 text-[11px] font-semibold text-[#B5B9C4] transition-colors hover:text-foreground"
                  >
                    <ExternalLink className="size-3" />
                    {t("detail.viewOnNyaa")}
                  </button>
                  {!s.is_active && (
                    <button
                      type="button"
                      disabled={switchSource.isPending}
                      onClick={() => switchSource.mutate(s.source_item_id)}
                      className="ml-auto rounded-[8px] bg-primary px-2.5 py-1.5 text-[11px] font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
                    >
                      {switchSource.isPending ? t("detail.switching") : t("detail.useSource")}
                    </button>
                  )}
                </div>
              </div>
            ))
          )}
        </div>

        <div className="flex items-center justify-end border-t border-[#1E212A] px-[22px] py-[14px]">
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="rounded-[10px] border border-[#33374A] px-[18px] py-2.5 text-[13px] font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
          >
            {t("common.close")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
