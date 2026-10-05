import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, ChevronDown, Eraser, FolderOpen, Inbox, Package, Pause, Play, Users, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import { useNavigate } from "react-router-dom";
import { getStorageStats } from "@/lib/stats";
import { formatBytes } from "@/lib/format";
import {
  cancelEpisodeDownload,
  listEpisodeSources,
  listRecentEpisodes,
  packKey,
  onDownloadProgress,
  pauseEpisodeDownload,
  resumeEpisodeDownload,
  type DownloadProgress,
  type Episode,
} from "@/lib/episodes";
import { listWatches } from "@/lib/watches";
import { parseEpisodeLabel, episodeLabelOf } from "@/lib/episodeName";

const DISMISSED_STORAGE_KEY = "torii:dismissed-downloads";

function useDismissedDownloads() {
  const [dismissed, setDismissed] = useState<Set<number>>(() => {
    try {
      const raw = localStorage.getItem(DISMISSED_STORAGE_KEY);
      return raw ? new Set(JSON.parse(raw)) : new Set();
    } catch {
      return new Set();
    }
  });

  function dismiss(episodeId: number) {
    setDismissed((prev) => {
      const next = new Set(prev);
      next.add(episodeId);
      try {
        localStorage.setItem(DISMISSED_STORAGE_KEY, JSON.stringify([...next]));
      } catch {
      }
      return next;
    });
  }

  function dismissMany(episodeIds: number[]) {
    setDismissed((prev) => {
      const next = new Set([...prev, ...episodeIds]);
      try {
        localStorage.setItem(DISMISSED_STORAGE_KEY, JSON.stringify([...next]));
      } catch {
      }
      return next;
    });
  }

  return { dismissed, dismiss, dismissMany };
}

const FILTERS = [
  { id: "downloading", labelKey: "downloads.filterActive" },
  { id: "available", labelKey: "downloads.filterDone" },
  { id: "error", labelKey: "downloads.filterErrors" },
  { id: "all", labelKey: "downloads.filterAll" },
] as const;

function StatTile({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="flex flex-1 flex-col gap-2 rounded-[14px] border border-[#1E212A] bg-[#15171D] p-4">
      <span className="flex items-center gap-1.5 text-[11px] font-bold tracking-wide text-[#6C7180] uppercase">
        {icon}
        {label}
      </span>
      <span className="font-heading text-[22px] font-bold">{value}</span>
    </div>
  );
}

function IconBtn({
  label,
  onClick,
  color,
  children,
}: {
  label: string;
  onClick: () => void;
  color?: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex size-8 items-center justify-center rounded-[8px] border border-[#262A35] bg-transparent hover:bg-secondary"
      style={{ color: color ?? "#B5B9C4" }}
    >
      {children}
    </button>
  );
}

function StatusBadge({ status, paused }: { status: Episode["status"]; paused: boolean }) {
  const { t } = useTranslation();
  if (paused) {
    return (
      <span
        className="rounded-[5px] px-[7px] py-[3px] text-[9px] font-bold tracking-wide uppercase"
        style={{ color: "#D7D9DE", background: "rgba(255,255,255,0.12)" }}
      >
        {t("downloads.paused")}
      </span>
    );
  }
  const map: Record<string, { label: string; bg: string; fg: string }> = {
    downloading: { label: t("episodeStatus.downloading"), bg: "#FF6A45", fg: "#0B0C10" },
    available: { label: t("downloads.done"), bg: "#6FC48A", fg: "#0B0C10" },
    error: { label: t("episodeStatus.error"), bg: "#E5484D", fg: "#0B0C10" },
    found: { label: t("episodeStatus.found"), bg: "transparent", fg: "#B5B9C4" },
  };
  const s = map[status] ?? { label: status, bg: "transparent", fg: "#B5B9C4" };
  return (
    <span
      className="rounded-[5px] px-[7px] py-[3px] text-[9px] font-bold tracking-wide uppercase"
      style={{
        color: s.fg,
        background: s.bg,
        border: s.bg === "transparent" ? "1px solid #33374A" : undefined,
      }}
    >
      {s.label}
    </span>
  );
}

function EpisodeRow({
  episode,
  watchTitle,
  watchFormat,
  coverUrl,
  progress,
  onPause,
  onResume,
  onDismiss,
  onCancel,
  onWatch,
}: {
  episode: Episode;
  watchTitle: string;
  watchFormat: string | null;
  coverUrl: string | null;
  progress: DownloadProgress | undefined;
  onPause: () => void;
  onResume: () => void;
  onDismiss: () => void;
  onCancel: () => void;
  onWatch: () => void;
}) {
  const { t } = useTranslation();
  const totalBytes = progress?.total_bytes ?? 0;
  const progressBytes = progress?.progress_bytes ?? 0;
  const pct = totalBytes > 0 ? Math.min(100, (progressBytes / totalBytes) * 100) : 0;
  const isDownloading = episode.status === "downloading";
  const isPaused = isDownloading && progress?.state === "paused";

  return (
    <div
      className="flex items-center gap-4 rounded-[12px] border border-[#1E212A] bg-[#15171D] p-4"
      style={{ opacity: isPaused ? 0.75 : 1 }}
    >
      <div className="size-11 shrink-0 overflow-hidden rounded-[8px] bg-[#262A35]">
        {coverUrl && <img src={coverUrl} alt="" className="size-full object-cover" />}
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex items-center gap-2.5">
          <span className="truncate text-[13px] font-semibold">{watchTitle}</span>
          <StatusBadge status={episode.status} paused={!!isPaused} />
          <span className="ml-auto shrink-0 text-[12px] text-[#8A8F9C]">
            {isDownloading
              ? `${formatBytes(progressBytes)} / ${totalBytes > 0 ? formatBytes(totalBytes) : "?"}`
              : episode.status === "available" && totalBytes > 0
                ? formatBytes(totalBytes)
                : ""}
          </span>
        </div>
        <p className="truncate text-[11px] text-[#6C7180]" title={episode.name ?? undefined}>
          {episodeLabelOf({ format: watchFormat }, episode.name, episode.episode_number)}
        </p>

        {isDownloading && (
          <>
            <div className="h-1.5 overflow-hidden rounded-full bg-[#22252E]">
              <div
                className="h-full rounded-full"
                style={{ width: `${pct}%`, background: isPaused ? "#4E5361" : "#FF6A45" }}
              />
            </div>
            <div className="flex items-center gap-4 text-[11px] text-[#6C7180]">
              <span>{pct.toFixed(0)}%</span>
              {!isPaused && (
                <>
                  <span className="flex items-center gap-1">
                    <ArrowDown className="size-2.5" />
                    {progress?.download_speed_mbps ? `${progress.download_speed_mbps.toFixed(1)} MB/s` : "—"}
                  </span>
                  <span className="flex items-center gap-1">
                    <ArrowUp className="size-2.5" />
                    {progress?.upload_speed_mbps ? `${progress.upload_speed_mbps.toFixed(1)} MB/s` : "—"}
                  </span>
                  <span className="flex items-center gap-1">
                    <Users className="size-2.5" />
                    {progress?.peers ?? 0}
                  </span>
                  {progress?.eta_human && <span>ETA {progress.eta_human}</span>}
                </>
              )}
              {isPaused && <span>{t("downloads.pausedManually")}</span>}
            </div>
          </>
        )}

        {episode.status === "error" && episode.error_message && (
          <p className="truncate text-[11px] text-destructive">{episode.error_message}</p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-2">
        {isDownloading && !isPaused && (
          <IconBtn label={t("detail.watchNow")} onClick={onWatch} color="#FF6A45">
            <Play className="size-3.5" fill="currentColor" />
          </IconBtn>
        )}
        {isDownloading &&
          (isPaused ? (
            <IconBtn label={t("downloads.resume")} onClick={onResume} color="#FF6A45">
              <Play className="size-3.5" fill="currentColor" />
            </IconBtn>
          ) : (
            <IconBtn label={t("downloads.pause")} onClick={onPause}>
              <Pause className="size-3.5" fill="currentColor" />
            </IconBtn>
          ))}
        <IconBtn
          label={t("downloads.openFolder")}
          onClick={() => episode.save_path && openPath(episode.save_path)}
        >
          <FolderOpen className="size-3.5" />
        </IconBtn>
        {episode.status === "available" ? (
          <IconBtn label={t("downloads.removeFromHistory")} onClick={onDismiss}>
            <X className="size-3.5" />
          </IconBtn>
        ) : (
          <IconBtn label={t("downloads.cancel")} onClick={onCancel} color="#E5484D">
            <X className="size-3.5" />
          </IconBtn>
        )}
      </div>
    </div>
  );
}

function PackRow({
  episodes,
  watchTitle,
  coverUrl,
  progressByEpisode,
  onPause,
  onResume,
  onCancelAll,
  onDismissAll,
  onCancelOne,
  onWatch,
}: {
  episodes: Episode[];
  watchTitle: string;
  coverUrl: string | null;
  progressByEpisode: Record<number, DownloadProgress>;
  onPause: () => void;
  onResume: () => void;
  onCancelAll: () => void;
  onDismissAll: () => void;
  onCancelOne: (ep: Episode) => void;
  onWatch: (ep: Episode) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const first = episodes[0];
  const { data: sources } = useQuery({
    queryKey: ["episode-sources", first.id],
    queryFn: () => listEpisodeSources(first.id),
    staleTime: Infinity,
  });
  const packTitle = sources?.find((s) => s.source_item_id === first.source_item_id)?.title ?? first.name ?? "";
  const sorted = [...episodes].sort((a, b) => (a.episode_number ?? 0) - (b.episode_number ?? 0));
  const numbers = sorted.map((e) => e.episode_number).filter((n): n is number => n != null);
  const range = numbers.length ? `${numbers[0]}–${numbers[numbers.length - 1]}` : "";

  let done = 0;
  let total = 0;
  for (const e of episodes) {
    const p = progressByEpisode[e.id];
    if (p) {
      done += p.progress_bytes;
      total += p.total_bytes;
    }
  }
  const pct = total > 0 ? Math.min(100, (done / total) * 100) : 0;
  const live = episodes.map((e) => progressByEpisode[e.id]).find((p) => p != null);
  const downloading = episodes.some((e) => e.status === "downloading");
  const allDone = episodes.every((e) => e.status === "available");
  const paused = downloading && live?.state === "paused";
  const ready = episodes.filter((e) => e.status === "available").length;
  const failed = episodes.filter((e) => e.status === "error").length;

  return (
    <div className="rounded-[12px] border border-[#1E212A] bg-[#15171D]" style={{ opacity: paused ? 0.75 : 1 }}>
      <div className="flex items-center gap-4 p-4">
        <div className="size-11 shrink-0 overflow-hidden rounded-[8px] bg-[#262A35]">
          {coverUrl && <img src={coverUrl} alt="" className="size-full object-cover" />}
        </div>

        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex items-center gap-2.5">
            <span className="truncate text-[13px] font-semibold">{watchTitle}</span>
            <span className="flex shrink-0 items-center gap-1 rounded-[5px] bg-accent2 px-[7px] py-[3px] text-[9px] font-bold tracking-wide text-[#0B0C10] uppercase">
              <Package className="size-2.5" />
              {t("downloads.pack", { count: episodes.length })}
            </span>
            <StatusBadge status={allDone ? "available" : failed === episodes.length ? "error" : "downloading"} paused={!!paused} />
            <span className="ml-auto shrink-0 text-[12px] text-[#8A8F9C]">
              {total > 0 ? (allDone ? formatBytes(total) : `${formatBytes(done)} / ${formatBytes(total)}`) : ""}
            </span>
          </div>
          <p className="truncate text-[11px] text-[#6C7180]" title={packTitle}>
            {range && `${t("downloads.packEpisodes", { range })} · `}
            {packTitle}
          </p>

          {!allDone && (
            <>
              <div className="h-1.5 overflow-hidden rounded-full bg-[#22252E]">
                <div className="h-full rounded-full" style={{ width: `${pct}%`, background: paused ? "#4E5361" : "#FF6A45" }} />
              </div>
              <div className="flex items-center gap-4 text-[11px] text-[#6C7180]">
                <span>{t("downloads.packReady", { ready, total: episodes.length })}</span>
                <span>{pct.toFixed(0)}%</span>
                {!paused && live && (
                  <>
                    <span className="flex items-center gap-1">
                      <ArrowDown className="size-2.5" />
                      {live.download_speed_mbps ? `${live.download_speed_mbps.toFixed(1)} MB/s` : "—"}
                    </span>
                    <span className="flex items-center gap-1">
                      <ArrowUp className="size-2.5" />
                      {live.upload_speed_mbps ? `${live.upload_speed_mbps.toFixed(1)} MB/s` : "—"}
                    </span>
                    <span className="flex items-center gap-1">
                      <Users className="size-2.5" />
                      {live.peers ?? 0}
                    </span>
                  </>
                )}
                {paused && <span>{t("downloads.pausedManually")}</span>}
                {failed > 0 && <span className="text-destructive">{t("downloads.packFailed", { count: failed })}</span>}
              </div>
            </>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {downloading &&
            (paused ? (
              <IconBtn label={t("downloads.resume")} onClick={onResume} color="#FF6A45">
                <Play className="size-3.5" fill="currentColor" />
              </IconBtn>
            ) : (
              <IconBtn label={t("downloads.pause")} onClick={onPause}>
                <Pause className="size-3.5" fill="currentColor" />
              </IconBtn>
            ))}
          <IconBtn label={t("downloads.openFolder")} onClick={() => first.save_path && openPath(first.save_path)}>
            <FolderOpen className="size-3.5" />
          </IconBtn>
          {allDone ? (
            <IconBtn label={t("downloads.removeFromHistory")} onClick={onDismissAll}>
              <X className="size-3.5" />
            </IconBtn>
          ) : (
            <IconBtn label={t("downloads.cancelPack")} onClick={onCancelAll} color="#E5484D">
              <X className="size-3.5" />
            </IconBtn>
          )}
          <IconBtn label={open ? t("downloads.hideEpisodes") : t("downloads.showEpisodes")} onClick={() => setOpen((v) => !v)}>
            <ChevronDown className={`size-3.5 transition-transform ${open ? "rotate-180" : ""}`} />
          </IconBtn>
        </div>
      </div>

      {open && (
        <div className="flex flex-col gap-1 border-t border-[#1E212A] px-4 py-3">
          {sorted.map((ep) => {
            const p = progressByEpisode[ep.id];
            const epPct = p && p.total_bytes > 0 ? Math.min(100, (p.progress_bytes / p.total_bytes) * 100) : ep.status === "available" ? 100 : 0;
            return (
              <div key={ep.id} className="flex items-center gap-3 rounded-lg px-2 py-1.5 hover:bg-white/[0.03]">
                <span className="w-24 shrink-0 truncate text-[12px] font-medium" title={ep.name ?? undefined}>
                  {parseEpisodeLabel(ep.name, ep.episode_number)}
                </span>
                <StatusBadge status={ep.status} paused={false} />
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-[#22252E]">
                  <div
                    className="h-full rounded-full"
                    style={{ width: `${epPct}%`, background: ep.status === "available" ? "#6FC48A" : "#FF6A45" }}
                  />
                </div>
                <span className="w-9 shrink-0 text-right text-[11px] text-[#6C7180]">{epPct.toFixed(0)}%</span>
                {(ep.status === "downloading" || ep.status === "available") && (
                  <IconBtn label={t("detail.watchNow")} onClick={() => onWatch(ep)} color="#FF6A45">
                    <Play className="size-3" fill="currentColor" />
                  </IconBtn>
                )}
                {ep.status !== "available" && (
                  <IconBtn label={t("downloads.cancel")} onClick={() => onCancelOne(ep)} color="#E5484D">
                    <X className="size-3" />
                  </IconBtn>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default function Downloads() {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("all");
  const [progressByEpisode, setProgressByEpisode] = useState<Record<number, DownloadProgress>>({});
  const { dismissed, dismiss, dismissMany } = useDismissedDownloads();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: storage } = useQuery({ queryKey: ["storage-stats"], queryFn: getStorageStats });
  const { data: episodes = [] } = useQuery({
    queryKey: ["recent-episodes"],
    queryFn: listRecentEpisodes,
    refetchInterval: 10_000,
  });
  const { data: watches = [] } = useQuery({ queryKey: ["watches"], queryFn: listWatches });

  useEffect(() => {
    const unlisten = onDownloadProgress((batch) => {
      setProgressByEpisode((prev) => {
        const next = { ...prev };
        for (const p of batch) next[p.episode_id] = p;
        return next;
      });
      if (batch.some((p) => p.finished)) {
        queryClient.invalidateQueries({ queryKey: ["recent-episodes"] });
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  const watchById = useMemo(() => {
    const map = new Map<number, { title: string; cover_url: string | null; format: string | null }>();
    for (const w of watches) map.set(w.id, { title: w.title, cover_url: w.cover_url, format: w.format });
    return map;
  }, [watches]);

  const visibleEpisodes = useMemo(
    () => episodes.filter((e) => e.status !== "deleted" && e.status !== "pending" && !dismissed.has(e.id)),
    [episodes, dismissed],
  );

  const counts = useMemo(() => {
    const c: Record<string, number> = { downloading: 0, available: 0, error: 0 };
    for (const e of visibleEpisodes) c[e.status] = (c[e.status] ?? 0) + 1;
    return c;
  }, [visibleEpisodes]);

  const filtered = filter === "all" ? visibleEpisodes : visibleEpisodes.filter((e) => e.status === filter);

  const clearable = visibleEpisodes.filter((e) => e.status === "available");

  // A pack reports the same torrent speed on each of its episodes: count it once.
  const torrentProgress = useMemo(() => {
    const byTorrent = new Map<string, DownloadProgress>();
    for (const e of episodes) {
      const p = progressByEpisode[e.id];
      const key = packKey(e) ?? `episode:${e.id}`;
      if (p && !byTorrent.has(key)) byTorrent.set(key, p);
    }
    return [...byTorrent.values()];
  }, [episodes, progressByEpisode]);
  const totalDownloadSpeed = torrentProgress.reduce((sum, p) => sum + (p.download_speed_mbps ?? 0), 0);
  const totalUploadSpeed = torrentProgress.reduce((sum, p) => sum + (p.upload_speed_mbps ?? 0), 0);
  const downloadingTorrents = new Set(
    visibleEpisodes.filter((e) => e.status === "downloading").map((e) => packKey(e) ?? `episode:${e.id}`),
  ).size;

  const rows = useMemo(() => {
    const out: { key: string; episodes: Episode[] }[] = [];
    const index = new Map<string, number>();
    for (const e of filtered) {
      const key = packKey(e);
      if (key && index.has(key)) {
        out[index.get(key)!].episodes.push(e);
      } else {
        if (key) index.set(key, out.length);
        out.push({ key: key ?? `episode:${e.id}`, episodes: [e] });
      }
    }
    return out;
  }, [filtered]);

  const watchEpisode = (ep: Episode) => {
    if (ep.episode_number != null) navigate(`/watch/${ep.watch_id}/${ep.episode_number}`);
  };
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["recent-episodes"] });

  return (
    <div className="flex flex-col gap-[22px]">
      <div className="flex items-baseline justify-between">
        <div className="flex items-baseline gap-3">
          <h1 className="text-[26px] font-bold">{t("nav.downloads")}</h1>
          <span className="text-[13px] text-[#6C7180]">{t("downloads.itemCount", { count: visibleEpisodes.length })}</span>
        </div>
      </div>

      <div className="flex gap-3.5">
        <StatTile
          label={t("downloads.downloadingNow")}
          value={t("downloads.torrentCount", { count: downloadingTorrents })}
        />
        <StatTile
          label={t("downloads.downloadSpeed")}
          value={`${totalDownloadSpeed.toFixed(1)} MB/s`}
          icon={<ArrowDown className="size-3" />}
        />
        <StatTile
          label={t("downloads.uploadSpeed")}
          value={`${totalUploadSpeed.toFixed(1)} MB/s`}
          icon={<ArrowUp className="size-3" />}
        />
        <StatTile label={t("downloads.spaceUsed")} value={formatBytes(storage?.used_bytes ?? 0)} />
      </div>

      <div className="flex items-center justify-between gap-3">
        <nav aria-label={t("downloads.filter")} className="flex items-center gap-2">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            type="button"
            aria-pressed={filter === f.id}
            onClick={() => setFilter(f.id)}
            className={`rounded-full px-4 py-2 text-[13px] font-semibold transition-colors ${
              filter === f.id
                ? "bg-primary text-primary-foreground"
                : "border border-[#23262F] text-[#B5B9C4] hover:text-foreground"
            }`}
          >
            {t(f.labelKey)} · {f.id === "all" ? visibleEpisodes.length : (counts[f.id] ?? 0)}
          </button>
        ))}
        </nav>
        <button
          type="button"
          disabled={clearable.length === 0}
          onClick={() => dismissMany(clearable.map((e) => e.id))}
          className="flex items-center gap-2 rounded-full border border-[#23262F] px-4 py-2 text-[13px] font-semibold text-[#B5B9C4] transition-colors hover:text-foreground disabled:opacity-40"
        >
          <Eraser className="size-3.5" />
          {t("downloads.clearHistory")}
        </button>
      </div>

      {filtered.length === 0 ? (
        <div className="flex flex-col items-center gap-3 rounded-[14px] border border-dashed border-[#23262F] py-16 text-center">
          <span className="flex size-12 items-center justify-center rounded-full bg-secondary text-[#6C7180]">
            <Inbox className="size-5" />
          </span>
          <p className="text-sm font-semibold">{t("downloads.emptyTitle")}</p>
          <p className="max-w-sm text-xs text-[#6C7180]">{t("downloads.emptyText")}</p>
        </div>
      ) : (
        <div className="flex flex-col gap-2.5">
          {rows.map(({ key, episodes: group }) => {
            const episode = group[0];
            const watch = watchById.get(episode.watch_id);
            if (packKey(episode) && group.length > 1) {
              const active = group.find((e) => e.status === "downloading") ?? episode;
              return (
                <PackRow
                  key={key}
                  episodes={group}
                  watchTitle={watch?.title ?? "Anime"}
                  coverUrl={watch?.cover_url ?? null}
                  progressByEpisode={progressByEpisode}
                  onPause={() => pauseEpisodeDownload(active.id)}
                  onResume={() => resumeEpisodeDownload(active.id)}
                  onDismissAll={() => dismissMany(group.map((e) => e.id))}
                  onCancelAll={async () => {
                    for (const e of group.filter((e) => e.status !== "available")) {
                      await cancelEpisodeDownload(e.id, true).catch(() => {});
                    }
                    refresh();
                  }}
                  onCancelOne={(e) => cancelEpisodeDownload(e.id, true).then(refresh)}
                  onWatch={watchEpisode}
                />
              );
            }
            return (
              <EpisodeRow
                key={episode.id}
                episode={episode}
                watchTitle={watch?.title ?? "Anime"}
                watchFormat={watch?.format ?? null}
                coverUrl={watch?.cover_url ?? null}
                progress={progressByEpisode[episode.id]}
                onPause={() => pauseEpisodeDownload(episode.id)}
                onResume={() => resumeEpisodeDownload(episode.id)}
                onDismiss={() => dismiss(episode.id)}
                onWatch={() => watchEpisode(episode)}
                onCancel={() =>
                  cancelEpisodeDownload(episode.id, true).then(() =>
                    queryClient.invalidateQueries({ queryKey: ["recent-episodes"] }),
                  )
                }
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
