import { Bookmark, Check, Info, Loader2, Play, Star } from "lucide-react";
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
  /** "Continuar" (hover): abre o player continuando de onde parou. */
  onPlay: () => void;
  /** "Ver anime" (hover): abre a página do anime. */
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
      <div
        className="group relative overflow-hidden rounded-xl bg-secondary shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow duration-200 hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] hover:ring-white/15"
        style={{ width, height }}
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
          className={`absolute inset-0 flex items-center justify-center gap-2 bg-black/55 backdrop-blur-[2px] transition-opacity duration-200 group-focus-within:opacity-100 group-hover:opacity-100 ${
            playLoading ? "opacity-100" : "opacity-0"
          }`}
        >
          <button
            type="button"
            onClick={onPlay}
            aria-label={t("library.watch", { title: displayTitle })}
            className="flex items-center gap-1.5 rounded-lg bg-white px-3.5 py-2 text-xs font-semibold text-black transition-opacity hover:opacity-90"
          >
            {playLoading ? <Loader2 className="size-3.5 animate-spin" /> : <Play className="size-3.5 fill-black" />}
            {t("home.continue")}
          </button>
          <button
            type="button"
            onClick={onOpenDetails}
            aria-label={t("library.openPageOf", { title: displayTitle })}
            className="flex items-center gap-1.5 rounded-lg border border-white/30 bg-black/40 px-3.5 py-2 text-xs font-semibold text-white transition-colors hover:bg-white/15"
          >
            <Info className="size-3.5" />
            {t("home.viewAnime")}
          </button>
        </div>
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
