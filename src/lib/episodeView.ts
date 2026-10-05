import { t } from "@/i18n";
import { currentLocale } from "@/i18n";
import { languageLabel, qualityLabel } from "@/lib/constants";
import type { Episode } from "@/lib/episodes";
import type { Watch } from "@/lib/watches";

export type Tone =
  | "watched"
  | "progress"
  | "downloading"
  | "new"
  | "ready"
  | "stream"
  | "waiting"
  | "searching"
  | "airing"
  | "removed"
  | "error"
  | "muted";

export type EpisodeGroup = "watched" | "ready" | "waiting" | "other";

export interface EpisodeView {
  label: string;
  tone: Tone;
  /** Small second line: file state, missing filters, error. */
  detail?: string;
  /** Watch progress 0–1, for the bar under in-progress episodes. */
  watchProgress?: number;
  group: EpisodeGroup;
  playable: boolean;
}

const NEW_FOR_MS = 48 * 3600 * 1000;
const MIN_PROGRESS_MS = 5_000;

function clock(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** The anime's filters still waiting to be met, e.g. "audio Portuguese (Brazil) · 1080p". */
export function pendingFilters(watch: Watch): string[] {
  const out: string[] = [];
  for (const lang of watch.audio_lang?.split(",").filter(Boolean) ?? []) {
    out.push(t("epView.audioFilter", { lang: languageLabel(lang) }));
  }
  for (const lang of watch.sub_lang?.split(",").filter(Boolean) ?? []) {
    out.push(t("epView.subtitleFilter", { lang: languageLabel(lang) }));
  }
  if (watch.quality && watch.quality !== "any") out.push(qualityLabel(watch.quality));
  return out;
}

function fileDetail(ep: Episode, downloadPct?: number): string | undefined {
  switch (ep.status) {
    case "available":
      return t("epView.fileOnDisk");
    case "downloading":
      return t("epView.fileDownloading", { pct: Math.round(downloadPct ?? 0) });
    case "deleted":
      return t("epView.fileDeleted");
    default:
      return undefined;
  }
}

/// Watching comes first: a watched episode reads "Watched" even if its file is gone.
export function episodeView(
  ep: Episode,
  watch: Watch,
  ctx: { airingAt?: number; durationMs?: number; downloadPct?: number; now: number },
): EpisodeView {
  const n = ep.episode_number;
  const outOfRange =
    ep.status === "pending" &&
    n != null &&
    ((watch.episode_start != null && n < watch.episode_start) || (watch.episode_end != null && n > watch.episode_end));
  const streamable = watch.streaming && ["ready", "found", "downloading", "error", "deleted"].includes(ep.status);
  const playable = n != null && (ep.status === "available" || ep.status === "downloading" || streamable);
  const position = ep.watch_position_ms ?? 0;

  if (outOfRange) return { label: t("epView.outOfRange"), tone: "muted", group: "other", playable: false };

  if (ep.watched_at) {
    return { label: t("epView.watched"), tone: "watched", detail: fileDetail(ep, ctx.downloadPct), group: "watched", playable };
  }
  if (position > MIN_PROGRESS_MS && playable) {
    return {
      label: t("epView.stoppedAt", { time: clock(position) }),
      tone: "progress",
      detail: fileDetail(ep, ctx.downloadPct),
      watchProgress: ctx.durationMs ? Math.min(1, position / ctx.durationMs) : undefined,
      group: "ready",
      playable,
    };
  }

  switch (ep.status) {
    case "downloading":
      return {
        label: t("epView.downloading", { pct: Math.round(ctx.downloadPct ?? 0) }),
        tone: "downloading",
        group: "waiting",
        playable,
      };
    case "found":
      return { label: t("epView.starting"), tone: "downloading", group: "waiting", playable };
    case "available": {
      const fresh = ep.available_at != null && ctx.now - new Date(ep.available_at).getTime() < NEW_FOR_MS;
      return { label: fresh ? t("epView.new") : t("epView.ready"), tone: fresh ? "new" : "ready", group: "ready", playable };
    }
    case "ready":
      return { label: t("epView.stream"), tone: "stream", group: "ready", playable };
    case "error":
      return { label: t("epView.error"), tone: "error", detail: ep.error_message ?? undefined, group: "waiting", playable };
    case "deleted":
      return watch.streaming
        ? { label: t("epView.stream"), tone: "stream", group: "ready", playable }
        : { label: t("epView.removed"), tone: "removed", group: "other", playable: false };
  }

  // pending
  if (ctx.airingAt != null && ctx.airingAt * 1000 > ctx.now) {
    const date = new Date(ctx.airingAt * 1000).toLocaleDateString(currentLocale(), { day: "2-digit", month: "2-digit" });
    return { label: t("epView.airs", { date }), tone: "airing", group: "waiting", playable: false };
  }
  const filters = pendingFilters(watch);
  if (filters.length > 0) {
    return {
      label: t("epView.waitingFilters"),
      tone: "waiting",
      detail: t("epView.waitingDetail", { filters: filters.join(" · ") }),
      group: "waiting",
      playable,
    };
  }
  return { label: t("epView.searching"), tone: "searching", group: "waiting", playable };
}

export const TONE_STYLE: Record<Tone, { bg: string; fg: string; border?: string }> = {
  watched: { bg: "transparent", fg: "#6FC48A", border: "#6FC48A66" },
  progress: { bg: "#FF6A4522", fg: "#FF6A45", border: "#FF6A4566" },
  downloading: { bg: "#FF6A45", fg: "#0B0C10" },
  new: { bg: "#FFB648", fg: "#0B0C10" },
  ready: { bg: "#6FC48A", fg: "#0B0C10" },
  stream: { bg: "transparent", fg: "#6FC48A", border: "#33374A" },
  waiting: { bg: "#FFB64818", fg: "#FFB648", border: "#FFB64855" },
  searching: { bg: "transparent", fg: "#8A8F9C", border: "#33374A" },
  airing: { bg: "transparent", fg: "#8A8F9C", border: "#33374A" },
  removed: { bg: "transparent", fg: "#6C7180", border: "#2A2E39" },
  error: { bg: "#E5484D", fg: "#0B0C10" },
  muted: { bg: "transparent", fg: "#4E5361", border: "#23262F" },
};
