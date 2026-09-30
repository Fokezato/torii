import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Check,
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
import { getAnimeById, getAnimeSeasons, seasonTabLabel, type AnimeSummary } from "@/lib/anilist";
import { WatchFormModal } from "@/components/anime/WatchFormModal";
import { findContinueEpisode, playEpisode } from "@/lib/continueWatching";
import {
  deleteEpisode,
  downloadMissingEpisodes,
  forceCheckEpisode,
  listEpisodeSources,
  listWatchEpisodes,
  packKey,
  switchEpisodeSource,
  type Episode,
} from "@/lib/episodes";
import { notify } from "@/lib/notify";
import { parseEpisodeLabel, parseEpisodeNumber, parseSeasonFromTitle } from "@/lib/episodeName";
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
  episodeStatusLabel,
  languageLabel,
  listStatusLabel,
  listStatusOptions,
  qualityLabel as qualityName,
  seasonLabel,
  statusLabel,
} from "@/lib/constants";
import { useTranslation } from "react-i18next";
import { currentLocale } from "@/i18n";

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

  const upcomingByEpisode = new Map((anime?.upcoming_episodes ?? []).map((u) => [u.episode, u.airing_at]));

  const { data: episodeList = [] } = useQuery({
    queryKey: ["watch-episodes", watch?.id],
    queryFn: () => listWatchEpisodes(watch!.id),
    enabled: watch != null,
    refetchInterval: 10_000,
  });

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
    anime?.duration ? t("common.minPerEpisode", { count: anime.duration }) : null,
    episodes ? t("common.episodeCount", { count: episodes }) : null,
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
            {episodes ? ` · ${episodes} eps` : ""}
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
          <p className="max-w-[760px] text-[13.5px] leading-relaxed text-[#C7CAD3]">{anime.description}</p>
        )}
      </div>

      <div className="flex flex-col gap-3.5">
        <div className="flex flex-wrap items-center gap-2.5">
          <h2 className="text-lg font-bold">{t("detail.episodes")}</h2>
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
          <span className="text-xs text-[#6C7180]">
            {episodes
              ? t("detail.downloadedOf", { count: downloadedCount, total: episodes })
              : t("detail.downloaded", { count: downloadedCount })}
          </span>
        </div>
        {episodeList.length === 0 ? (
          <div className="rounded-xl border border-dashed border-[#23262F] p-6 text-center text-xs text-[#6C7180]">
            {t("detail.noEpisodes")}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <PackBanners episodes={episodeList} />
            {episodeList
              .slice()
              .sort(
                (a, b) =>
                  (a.episode_number ?? parseEpisodeNumber(a.name) ?? 0) -
                  (b.episode_number ?? parseEpisodeNumber(b.name) ?? 0),
              )
              .map((ep) => (
                <EpisodeItem key={ep.id} episode={ep} watch={watch} airingAt={upcomingByEpisode.get(ep.episode_number ?? -1)} />
              ))}
          </div>
        )}
      </div>

      <WatchPreferencesDialog watch={prefsOpen ? watch : null} onOpenChange={setPrefsOpen} />
      <RemoveWatchDialog watch={removeOpen ? watch : null} onOpenChange={setRemoveOpen} onDone={onRemoved} />
      <WatchFormModal anime={addSeason} onOpenChange={(open) => !open && setAddSeason(null)} />
    </div>
  );
}

const EPISODE_STATUS_STYLE: Record<string, { bg: string; fg: string }> = {
  pending: { bg: "transparent", fg: "#6C7180" },
  found: { bg: "transparent", fg: "#B5B9C4" },
  ready: { bg: "transparent", fg: "#6FC48A" },
  downloading: { bg: "#FF6A45", fg: "#0B0C10" },
  available: { bg: "#6FC48A", fg: "#0B0C10" },
  error: { bg: "#E5484D", fg: "#0B0C10" },
  deleted: { bg: "transparent", fg: "#6C7180" },
};

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

function EpisodeItem({ episode, watch, airingAt }: { episode: Episode; watch: Watch; airingAt?: number }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const n = episode.episode_number;
  const outOfRange =
    episode.status === "pending" &&
    n != null &&
    ((watch.episode_start != null && n < watch.episode_start) || (watch.episode_end != null && n > watch.episode_end));

  const notYetAired = !outOfRange && episode.status === "pending" && airingAt != null;

  const wantedLangs = [...(watch.audio_lang?.split(",") ?? []), ...(watch.sub_lang?.split(",") ?? [])]
    .filter(Boolean)
    .map(languageLabel);
  const waitingForLocalizedRelease =
    episode.status === "pending" && !outOfRange && !notYetAired && wantedLangs.length > 0;

  const muted = { bg: "transparent", fg: "#6C7180" };
  const s = outOfRange
    ? { label: t("episodeStatus.notDownloaded"), ...muted }
    : notYetAired
    ? { label: t("episodeStatus.announced"), ...muted }
    : waitingForLocalizedRelease
      ? { label: t("episodeStatus.waitingLanguage"), ...muted }
      : watch.streaming && episode.status === "deleted"
        ? { label: episodeStatusLabel("ready"), ...EPISODE_STATUS_STYLE.ready }
        : {
            label: episodeStatusLabel(episode.status),
            ...(EPISODE_STATUS_STYLE[episode.status] ?? { bg: "transparent", fg: "#B5B9C4" }),
          };
  return (
    <div className="flex items-center gap-3 rounded-[10px] border border-[#1E212A] bg-[#15171D] px-4 py-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex min-w-0 items-center gap-2 text-[12.5px] font-medium" title={episode.name ?? undefined}>
          <span className="truncate">{parseEpisodeLabel(episode.name, episode.episode_number)}</span>
          {packKey(episode) && episode.status !== "deleted" && (
            <span className="flex shrink-0 items-center gap-1 rounded-[4px] bg-accent2/15 px-1.5 py-px text-[9px] font-bold tracking-wide text-accent2 uppercase">
              <Package className="size-2.5" />
              {t("detail.packBadge")}
            </span>
          )}
        </span>
        {episode.status === "error" && episode.error_message && (
          <span className="truncate text-[11px] text-destructive">{episode.error_message}</span>
        )}
        {notYetAired && (
          <span className="truncate text-[11px] text-[#6C7180]">
            {t("detail.availableFrom", { date: new Date(airingAt! * 1000).toLocaleDateString(currentLocale()) })}
          </span>
        )}
        {waitingForLocalizedRelease && (
          <span className="truncate text-[11px] text-[#6C7180]" title={t("detail.localizedHint")}>
            {t("detail.waitingLocalized", { languages: wantedLangs.join(", ") })}
          </span>
        )}
      </div>
      <span
        className="shrink-0 rounded-[5px] px-[7px] py-[3px] text-[9px] font-bold tracking-wide uppercase"
        style={{
          color: s.fg,
          background: s.bg,
          border: s.bg === "transparent" ? "1px solid #33374A" : undefined,
        }}
      >
        {s.label}
      </span>
      {n != null &&
        (episode.status === "downloading" ||
          (watch.streaming && !notYetAired && !outOfRange && episode.status !== "available")) && (
        <button
          type="button"
          onClick={() => navigate(`/watch/${watch.id}/${n}`)}
          title={t("detail.watchNowHint")}
          className="flex shrink-0 items-center gap-1.5 rounded-[8px] border border-[#262A35] px-2.5 py-1 text-[11px] font-semibold text-foreground transition-colors hover:bg-secondary"
        >
          <Play className="size-3" fill="currentColor" />
          {t("detail.watchNow")}
        </button>
      )}
      {!notYetAired && (
        <EpisodeActionsMenu episode={episode} watch={watch} outOfRange={outOfRange} />
      )}
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
