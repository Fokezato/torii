import { Bookmark, Check, Loader2, MoreHorizontal, Play, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Watch } from "@/lib/watches";
import { listStatusLabel } from "@/lib/constants";
import { formatShortDate } from "@/lib/format";

const BADGE_STYLE: Record<string, string> = {
  watching: "bg-primary text-primary-foreground",
  downloaded: "bg-[#6FC48A] text-background",
  completed: "bg-black/45 text-[#D7D9DE]",
  planning: "border border-white/45 bg-transparent text-white",
};

const BADGE_ICON: Record<string, typeof Check | undefined> = {
  downloaded: Check,
  planning: Bookmark,
};

export function LibraryCard({
  watch,
  onPlay,
  onOpenDetails,
  playLoading = false,
  width = 220,
  title,
  seasonCount = 1,
}: {
  watch: Watch;
  /** Clique no card: abre o player continuando de onde parou. */
  onPlay: () => void;
  /** "⋯" no canto (aparece no hover): abre a página do anime. */
  onOpenDetails: () => void;
  playLoading?: boolean;
  width?: number;
  /** Título de exibição, se diferente do `watch.title` (ex. nome base sem "Season N" quando agrupado). */
  title?: string;
  /** Quantas temporadas esse card representa — mostra um indicador quando > 1. */
  seasonCount?: number;
}) {
  const { t } = useTranslation();
  const label = listStatusLabel(watch.list_status);
  const Icon = BADGE_ICON[watch.list_status];
  const height = Math.round(width * (124 / 220));
  const displayTitle = title ?? watch.title;

  return (
    <article className="shrink-0 space-y-2.5" style={{ width }}>
      <div className="group relative" style={{ width, height }}>
      <button
        type="button"
        onClick={onPlay}
        aria-label={t("library.watch", { title: displayTitle })}
        style={{ width, height }}
        className="relative block overflow-hidden rounded-xl bg-secondary shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow duration-200 group-hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] group-hover:ring-white/15 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
      >
        {watch.cover_url ? (
          <img
            src={watch.cover_url}
            alt=""
            className="absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.05]"
          />
        ) : (
          <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-secondary to-muted p-2 text-center text-xs font-medium text-muted-foreground">
            {displayTitle}
          </div>
        )}
        <div className="absolute inset-0 bg-black/0 transition-colors duration-200 group-hover:bg-black/30" />

        <span
          className={`absolute top-2 left-2 flex items-center gap-1 rounded-[5px] px-1.5 py-0.5 text-[9px] font-bold tracking-wide uppercase ${BADGE_STYLE[watch.list_status] ?? "bg-black/45 text-white"}`}
        >
          {Icon && <Icon className="size-[9px]" strokeWidth={3} />}
          {label}
        </span>
        {!watch.active && watch.list_status === "watching" && (
          <span className="absolute top-2 right-2 rounded-[5px] bg-black/55 px-1.5 py-0.5 text-[9px] font-bold text-white">
            {t("library.pausedBadge")}
          </span>
        )}
        {seasonCount > 1 && (
          <span className="absolute bottom-2 left-2 rounded-[5px] bg-black/65 px-1.5 py-0.5 text-[9px] font-bold text-white">
            {t("library.seasonCount", { count: seasonCount })}
          </span>
        )}

        <div
          className={`absolute inset-0 flex items-center justify-center transition-opacity duration-200 group-hover:opacity-100 ${
            playLoading ? "opacity-100" : "opacity-0"
          }`}
        >
          <span className="flex size-8 items-center justify-center rounded-full bg-white/90">
            {playLoading ? (
              <Loader2 className="size-3.5 animate-spin text-black" />
            ) : (
              <Play className="size-3 translate-x-0.5 fill-black text-black" />
            )}
          </span>
        </div>
      </button>
      <button
        type="button"
        aria-label={t("library.openPageOf", { title: displayTitle })}
        title={t("library.openPage")}
        onClick={onOpenDetails}
        className="absolute right-2 bottom-2 flex size-7 items-center justify-center rounded-full bg-black/65 text-white opacity-0 transition-all duration-200 group-hover:opacity-100 hover:bg-black/85 focus-visible:opacity-100"
      >
        <MoreHorizontal className="size-4" />
      </button>
      </div>

      <div className="space-y-1.5">
        <h3 className="line-clamp-1 text-sm font-semibold">{displayTitle}</h3>
        <div className="flex items-center justify-between">
          {watch.rating != null ? (
            <span className="flex items-center gap-1 text-xs font-bold">
              <Star className="size-3 fill-accent2 text-accent2" />
              {watch.rating}
            </span>
          ) : (
            <span className="text-xs text-[#4E5361]">{t("library.noRating")}</span>
          )}
          <span className="text-xs text-[#6C7180]">{formatShortDate(watch.updated_at)}</span>
        </div>
        <div className="h-[6px] w-full overflow-hidden rounded-full bg-[#262A35]" />
        <p className="text-[10.5px] text-[#6C7180]">
          {watch.episodes ? t("common.episodeCount", { count: watch.episodes }) : t("library.episodesUnknown")}
        </p>
      </div>
    </article>
  );
}
