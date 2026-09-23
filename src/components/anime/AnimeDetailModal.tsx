import { Plus, Star, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import type { AnimeSummary } from "@/lib/anilist";
import { genreLabel, seasonLabel, statusLabel } from "@/lib/constants";
import { TranslatedText } from "@/components/shared/TranslatedText";

interface AnimeDetailModalProps {
  anime: AnimeSummary | null;
  onOpenChange: (open: boolean) => void;
  onAdd: (anime: AnimeSummary) => void;
}

export function AnimeDetailModal({ anime, onOpenChange, onAdd }: AnimeDetailModalProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={anime !== null} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "680px", width: "92vw" }}
        className="gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
      >
        {anime && (
          <>
            <div className="relative h-[260px] shrink-0 overflow-hidden bg-secondary">
              {(anime.banner_url ?? anime.cover_url) && (
                <img
                  src={anime.banner_url ?? anime.cover_url ?? undefined}
                  alt=""
                  className="absolute inset-0 h-full w-full object-cover"
                />
              )}
              <div className="absolute inset-0 bg-gradient-to-t from-[#15171D] via-[#15171D]/10 to-transparent" />
              {anime.status && (
                <span className="absolute top-4 left-5 rounded-md bg-accent2 px-2.5 py-1 text-[11px] font-bold tracking-wide text-background uppercase">
                  {statusLabel(anime.status)}
                </span>
              )}
              <button
                type="button"
                aria-label={t("common.close")}
                onClick={() => onOpenChange(false)}
                className="absolute top-3.5 right-3.5 flex size-8 items-center justify-center rounded-full bg-black/45 text-white transition-colors hover:bg-black/65"
              >
                <X className="size-[15px]" strokeWidth={2} />
              </button>
            </div>

            <div className="flex flex-col gap-4 px-8 py-[26px] pb-[30px]">
              <div className="flex items-start justify-between gap-4">
                <h2 className="text-[26px] leading-[1.15] font-bold">{anime.title}</h2>
                {anime.score != null && (
                  <div className="flex shrink-0 items-center gap-1.5 pt-1">
                    <Star className="size-[15px] fill-accent2 text-accent2" />
                    <span className="text-sm font-bold">{(anime.score / 10).toFixed(1)}</span>
                  </div>
                )}
              </div>

              <p className="text-[12.5px] text-[#8A8F9C]">
                {[
                  anime.season_year && anime.season
                    ? `${anime.season_year} · ${seasonLabel(anime.season)}`
                    : anime.season_year,
                  anime.studio,
                  anime.duration && t("common.minPerEpisode", { count: anime.duration }),
                  anime.episodes && t("common.episodeCount", { count: anime.episodes }),
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </p>

              {anime.genres.length > 0 && (
                <div className="flex flex-wrap items-center gap-2">
                  {anime.genres.map((g) => (
                    <span
                      key={g}
                      className="rounded-full border border-white/10 bg-white/[0.06] px-2.5 py-1 text-[11px] font-semibold text-[#B5B9C4]"
                    >
                      {genreLabel(g)}
                    </span>
                  ))}
                </div>
              )}

              {anime.description && (
                <TranslatedText text={anime.description} className="text-sm leading-[1.65] text-[#C7CAD3]" />
              )}

              <div className="mt-2 flex items-center justify-between">
                <button
                  type="button"
                  onClick={() => onOpenChange(false)}
                  className="px-1.5 py-3 text-[13px] font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
                >
                  {t("common.dismiss")}
                </button>
                <button
                  type="button"
                  onClick={() => onAdd(anime)}
                  className="flex items-center gap-2 rounded-[10px] bg-primary px-[22px] py-3 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90"
                >
                  <Plus className="size-4" strokeWidth={2.2} />
                  {t("common.addToLibrary")}
                </button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
