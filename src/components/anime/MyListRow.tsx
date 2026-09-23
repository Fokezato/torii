import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Info, Loader2, Play, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { groupBySeries, listWatches, type WatchGroup } from "@/lib/watches";
import { findContinueEpisode, playEpisode } from "@/lib/continueWatching";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { statusLabel } from "@/lib/constants";
import { timeAgo } from "@/lib/format";

/// Animes da Biblioteca (temporadas juntas num card só, igual a Biblioteca),
/// do atualizado mais recente pro mais antigo. No hover: continuar de onde
/// parou ou abrir a página do anime.
export function MyListRow() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data } = useQuery({ queryKey: ["watches"], queryFn: listWatches });
  const groups = useMemo(
    () =>
      groupBySeries(data ?? [])
        .sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
        .slice(0, 10),
    [data],
  );
  const [playingKey, setPlayingKey] = useState<string | null>(null);

  if (groups.length === 0) return null;

  const openAnime = (g: WatchGroup) => navigate(`/library/${g.representative.id}`);

  // Nada baixado ainda → abre a página do anime.
  async function continueGroup(g: WatchGroup) {
    if (playingKey) return;
    setPlayingKey(g.key);
    try {
      const target = await findContinueEpisode(g.seasons);
      if (target) await playEpisode(target.watch, target.episode, navigate);
      else openAnime(g);
    } catch {
      openAnime(g);
    } finally {
      setPlayingKey(null);
    }
  }

  return (
    <section className="space-y-3">
      <div className="flex items-center gap-2.5">
        <span className="h-4 w-1 rounded-full bg-primary" />
        <h2 className="text-lg font-semibold tracking-tight">{t("home.myList")}</h2>
      </div>
      <ScrollArea className="w-full whitespace-nowrap">
        <div className="flex gap-4 pb-4">
          {groups.map((g) => {
            const w = g.representative;
            const paused = g.seasons.every((s) => !s.active);
            const rating = g.seasons.find((s) => s.rating != null)?.rating ?? null;
            return (
              <div
                key={g.key}
                className="group relative h-36 w-64 shrink-0 overflow-hidden rounded-xl bg-secondary text-left shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] hover:ring-white/15"
              >
                {w.cover_url ? (
                  <img src={w.cover_url} alt="" className="absolute inset-0 h-full w-full object-cover" />
                ) : (
                  <div className="absolute inset-0 bg-gradient-to-br from-secondary to-muted" />
                )}
                <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/10 to-transparent" />

                {w.status && (
                  <span
                    className={`absolute top-2 right-2 rounded-md px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide ${
                      w.status === "FINISHED"
                        ? "bg-primary text-primary-foreground"
                        : "bg-black/55 text-white backdrop-blur-sm"
                    }`}
                  >
                    {statusLabel(w.status)}
                  </span>
                )}

                <div className="absolute inset-x-0 bottom-0 p-3">
                  <p className="line-clamp-1 text-sm font-semibold text-white">{g.title}</p>
                  <div className="mt-0.5 flex items-center gap-2 text-xs text-white/65">
                    <span>{t("home.updated", { when: timeAgo(g.updated_at) })}</span>
                    {rating != null && (
                      <span className="flex items-center gap-0.5">
                        <Star className="size-3 fill-yellow-400 text-yellow-400" />
                        {rating}
                      </span>
                    )}
                    {paused && <span>· {t("home.paused")}</span>}
                  </div>
                </div>

                <div className="absolute inset-0 flex items-center justify-center gap-2 bg-black/55 opacity-0 backdrop-blur-[2px] transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                  <button
                    type="button"
                    onClick={() => continueGroup(g)}
                    className="flex items-center gap-1.5 rounded-lg bg-white px-3.5 py-2 text-xs font-semibold text-black transition-opacity hover:opacity-90"
                  >
                    {playingKey === g.key ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <Play className="size-3.5 fill-black" />
                    )}
                    {t("home.continue")}
                  </button>
                  <button
                    type="button"
                    onClick={() => openAnime(g)}
                    className="flex items-center gap-1.5 rounded-lg border border-white/30 bg-black/40 px-3.5 py-2 text-xs font-semibold text-white transition-colors hover:bg-white/15"
                  >
                    <Info className="size-3.5" />
                    {t("home.viewAnime")}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
        <ScrollBar orientation="horizontal" />
      </ScrollArea>
    </section>
  );
}
