import { motion } from "framer-motion";
import { Play } from "lucide-react";
import type { AnimeSummary } from "@/lib/anilist";
import { STATUS_LABEL } from "@/lib/constants";

interface AnimeCardProps {
  anime: AnimeSummary;
  onClick?: () => void;
}

export function AnimeCard({ anime, onClick }: AnimeCardProps) {
  return (
    <div className="w-[168px] shrink-0 space-y-2.5">
      <motion.button
        type="button"
        onClick={onClick}
        whileHover={{ scale: 1.03, y: -4 }}
        whileTap={{ scale: 0.98 }}
        transition={{ type: "spring", stiffness: 320, damping: 24 }}
        className="group relative block h-[236px] w-[168px] overflow-hidden rounded-xl bg-secondary shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow duration-200 hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] hover:ring-white/15 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
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

        <div className="absolute inset-0 bg-black/0 transition-colors duration-200 group-hover:bg-black/35" />

        <div className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-200 group-hover:opacity-100">
          <span className="flex size-10 items-center justify-center rounded-full bg-white/95 shadow-lg">
            <Play className="size-4 translate-x-0.5 fill-black text-black" />
          </span>
        </div>

        {anime.score != null && (
          <span className="absolute right-2 top-2 rounded-[5px] bg-black/45 px-1.5 py-[3px] text-[10px] font-bold text-white">
            ★ {(anime.score / 10).toFixed(1)}
          </span>
        )}
      </motion.button>

      <div className="flex flex-col gap-[3px]">
        <h3 className="line-clamp-1 text-sm leading-tight font-semibold">{anime.title}</h3>
        <p className="text-[11px] text-[#6C7180]">
          {anime.episodes ? `${anime.episodes} eps` : "? eps"}
          {anime.status ? ` · ${STATUS_LABEL[anime.status] ?? anime.status}` : ""}
        </p>
      </div>
    </div>
  );
}
