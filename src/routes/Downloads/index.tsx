import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, FolderOpen, Inbox, Pause, Play, Users, X } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { getStorageStats } from "@/lib/stats";
import { formatBytes } from "@/lib/format";
import {
  cancelEpisodeDownload,
  listRecentEpisodes,
  onDownloadProgress,
  pauseEpisodeDownload,
  resumeEpisodeDownload,
  type DownloadProgress,
  type Episode,
} from "@/lib/episodes";
import { listWatches } from "@/lib/watches";
import { parseEpisodeLabel } from "@/lib/episodeName";

const DISMISSED_STORAGE_KEY = "torii:dismissed-downloads";

// X num episódio já CONCLUÍDO só tira da visão de downloads (histórico) —
// nunca mexe no arquivo nem some da Biblioteca (bug real reportado: "limpo
// download e some da biblioteca"). Guardado local por viewer, não precisa
// ser estado do app. Episódio ainda baixando/na fila usa `cancelEpisodeDownload`
// de verdade em vez disso (ver `onCancel` no EpisodeRow) — aí não tem nada
// "concluído" pra preservar, então cancelar o torrent é o esperado.
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
        // best-effort
      }
      return next;
    });
  }

  return { dismissed, dismiss };
}

const FILTERS = [
  { id: "downloading", label: "Ativos" },
  { id: "available", label: "Concluídos" },
  { id: "error", label: "Erros" },
  { id: "all", label: "Todos" },
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
  if (paused) {
    return (
      <span
        className="rounded-[5px] px-[7px] py-[3px] text-[9px] font-bold tracking-wide uppercase"
        style={{ color: "#D7D9DE", background: "rgba(255,255,255,0.12)" }}
      >
        Pausado
      </span>
    );
  }
  const map: Record<string, { label: string; bg: string; fg: string }> = {
    downloading: { label: "Baixando", bg: "#FF6A45", fg: "#0B0C10" },
    available: { label: "Concluído", bg: "#6FC48A", fg: "#0B0C10" },
    error: { label: "Erro", bg: "#E5484D", fg: "#0B0C10" },
    found: { label: "Iniciando", bg: "transparent", fg: "#B5B9C4" },
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
  coverUrl,
  progress,
  onPause,
  onResume,
  onDismiss,
  onCancel,
}: {
  episode: Episode;
  watchTitle: string;
  coverUrl: string | null;
  progress: DownloadProgress | undefined;
  onPause: () => void;
  onResume: () => void;
  onDismiss: () => void;
  onCancel: () => void;
}) {
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
              : episode.status === "available"
                ? formatBytes(totalBytes)
                : ""}
          </span>
        </div>
        <p className="truncate text-[11px] text-[#6C7180]" title={episode.name ?? undefined}>
          {parseEpisodeLabel(episode.name)}
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
              {isPaused && <span>Pausado manualmente</span>}
            </div>
          </>
        )}

        {episode.status === "error" && episode.error_message && (
          <p className="truncate text-[11px] text-destructive">{episode.error_message}</p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-2">
        {isDownloading &&
          (isPaused ? (
            <IconBtn label="Retomar download" onClick={onResume} color="#FF6A45">
              <Play className="size-3.5" fill="currentColor" />
            </IconBtn>
          ) : (
            <IconBtn label="Pausar download" onClick={onPause}>
              <Pause className="size-3.5" fill="currentColor" />
            </IconBtn>
          ))}
        <IconBtn
          label="Abrir pasta"
          onClick={() => episode.save_path && openPath(episode.save_path)}
        >
          <FolderOpen className="size-3.5" />
        </IconBtn>
        {episode.status === "available" ? (
          <IconBtn label="Remover do histórico" onClick={onDismiss}>
            <X className="size-3.5" />
          </IconBtn>
        ) : (
          <IconBtn label="Cancelar download" onClick={onCancel} color="#E5484D">
            <X className="size-3.5" />
          </IconBtn>
        )}
      </div>
    </div>
  );
}

export default function Downloads() {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("all");
  const [progressByEpisode, setProgressByEpisode] = useState<Record<number, DownloadProgress>>({});
  const { dismissed, dismiss } = useDismissedDownloads();
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
    const map = new Map<number, { title: string; cover_url: string | null }>();
    for (const w of watches) map.set(w.id, { title: w.title, cover_url: w.cover_url });
    return map;
  }, [watches]);

  const visibleEpisodes = useMemo(
    // "pending" = placeholder que ainda nem foi procurado no Nyaa (ver
    // create_placeholder no Rust) — não é um download de verdade, não
    // pertence aqui, só na lista de episódios da Biblioteca.
    () => episodes.filter((e) => e.status !== "deleted" && e.status !== "pending" && !dismissed.has(e.id)),
    [episodes, dismissed],
  );

  const counts = useMemo(() => {
    const c: Record<string, number> = { downloading: 0, available: 0, error: 0 };
    for (const e of visibleEpisodes) c[e.status] = (c[e.status] ?? 0) + 1;
    return c;
  }, [visibleEpisodes]);

  const filtered = filter === "all" ? visibleEpisodes : visibleEpisodes.filter((e) => e.status === filter);

  const totalDownloadSpeed = Object.values(progressByEpisode).reduce(
    (sum, p) => sum + (p.download_speed_mbps ?? 0),
    0,
  );
  const totalUploadSpeed = Object.values(progressByEpisode).reduce(
    (sum, p) => sum + (p.upload_speed_mbps ?? 0),
    0,
  );

  return (
    <div className="flex flex-col gap-[22px]">
      <div className="flex items-baseline justify-between">
        <div className="flex items-baseline gap-3">
          <h1 className="text-[26px] font-bold">Downloads</h1>
          <span className="text-[13px] text-[#6C7180]">{visibleEpisodes.length} itens</span>
        </div>
      </div>

      <div className="flex gap-3.5">
        <StatTile label="Baixando agora" value={`${counts.downloading ?? 0} torrents`} />
        <StatTile
          label="Velocidade de download"
          value={`${totalDownloadSpeed.toFixed(1)} MB/s`}
          icon={<ArrowDown className="size-3" />}
        />
        <StatTile
          label="Velocidade de upload"
          value={`${totalUploadSpeed.toFixed(1)} MB/s`}
          icon={<ArrowUp className="size-3" />}
        />
        <StatTile label="Espaço usado" value={formatBytes(storage?.used_bytes ?? 0)} />
      </div>

      <nav aria-label="Filtrar downloads" className="flex items-center gap-2">
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
            {f.label} · {f.id === "all" ? visibleEpisodes.length : (counts[f.id] ?? 0)}
          </button>
        ))}
      </nav>

      {filtered.length === 0 ? (
        <div className="flex flex-col items-center gap-3 rounded-[14px] border border-dashed border-[#23262F] py-16 text-center">
          <span className="flex size-12 items-center justify-center rounded-full bg-secondary text-[#6C7180]">
            <Inbox className="size-5" />
          </span>
          <p className="text-sm font-semibold">Nenhum torrent por aqui</p>
          <p className="max-w-sm text-xs text-[#6C7180]">
            Assim que um episódio novo for encontrado pra um anime da sua biblioteca, o download começa
            automaticamente e aparece aqui.
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-2.5">
          {filtered.map((episode) => {
            const watch = watchById.get(episode.watch_id);
            return (
              <EpisodeRow
                key={episode.id}
                episode={episode}
                watchTitle={watch?.title ?? "Anime"}
                coverUrl={watch?.cover_url ?? null}
                progress={progressByEpisode[episode.id]}
                onPause={() => pauseEpisodeDownload(episode.id)}
                onResume={() => resumeEpisodeDownload(episode.id)}
                onDismiss={() => dismiss(episode.id)}
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
