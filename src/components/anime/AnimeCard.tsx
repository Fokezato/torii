import { motion } from "framer-motion";
import { Info, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AnimeSummary } from "@/lib/anilist";
import { statusLabel } from "@/lib/constants";

interface AnimeCardProps {
  anime: AnimeSummary;
  /** "+ Adicionar" (hover): abre o formulário de adicionar à biblioteca. */
  onAdd?: () => void;
  /** "Detalhes" (hover): abre as informações do anime. */
  onDetails?: () => void;
}

export function AnimeCard({ anime, onAdd, onDetails }: AnimeCardProps) {
  const { t } = useTranslation();
  return (
    <div className="w-[168px] shrink-0 space-y-2.5">
      <motion.div
        whileHover={{ scale: 1.03, y: -4 }}
        transition={{ type: "spring", stiffness: 320, damping: 24 }}
        className="group relative block h-[236px] w-[168px] overflow-hidden rounded-xl bg-secondary shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow duration-200 hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] hover:ring-white/15"
      >
        {anime.cover_url ? (
          <img
            src={anime.cover_url}
            alt={anime.title}
            loading="lazy"
            className="absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.06]"
          />
        ) : (
          <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-secondary to-muted p-3 text-center text-xs font-medium text-muted-foreground">
            {anime.title}
          </div>
        )}

        {anime.score != null && (
          <span className="absolute right-2 top-2 rounded-[5px] bg-black/45 px-1.5 py-[3px] text-[10px] font-bold text-white">
            ★ {(anime.score / 10).toFixed(1)}
          </span>
        )}

        <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-black/55 opacity-0 backdrop-blur-[2px] transition-opacity duration-200 group-focus-within:opacity-100 group-hover:opacity-100">
          <button
            type="button"
            onClick={onAdd}
            className="flex w-[120px] items-center justify-center gap-1.5 rounded-lg bg-white px-3 py-2 text-xs font-semibold text-black transition-opacity hover:opacity-90"
          >
            <Plus className="size-3.5" strokeWidth={2.5} />
            {t("addAnime.add")}
          </button>
          <button
            type="button"
            onClick={onDetails}
            className="flex w-[120px] items-center justify-center gap-1.5 rounded-lg border border-white/30 bg-black/40 px-3 py-2 text-xs font-semibold text-white transition-colors hover:bg-white/15"
          >
            <Info className="size-3.5" />
            {t("home.details")}
          </button>
        </div>
      </motion.div>

      <div className="flex flex-col gap-[3px]">
        <h3 className="line-clamp-1 text-sm leading-tight font-semibold">{anime.title}</h3>
        <p className="text-[11px] text-[#6C7180]">
          {anime.episodes ? `${anime.episodes} eps` : "? eps"}
          {anime.status ? ` · ${statusLabel(anime.status)}` : ""}
        </p>
      </div>
    </div>
  );
}
