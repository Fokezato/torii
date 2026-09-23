import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { playerOpen, playerStop } from "@/lib/player";
import { listWatchEpisodes } from "@/lib/episodes";
import { listWatches } from "@/lib/watches";
import { formatPlayerTitle, parseEpisodeLabel } from "@/lib/episodeName";
import { episodeNumberOf, isPlayable } from "@/lib/continueWatching";
import { usePlayerHost } from "@/lib/usePlayerHost";

// Progresso menor que isso = recomeça do início (não vale "continuar" de 3s).
const MIN_RESUME_MS = 5_000;

/// Player de verdade: janela inteira (fora do AppShell, sem barra lateral),
/// vídeo nativo do libvlc por baixo e os controles na janela de overlay por
/// cima (ver `routes/PlayerOverlay`). Abre o episódio `:episode` do anime
/// `:watchId`, continuando de onde parou se não tinha terminado.
export default function Player() {
  const { watchId, episode } = useParams<{ watchId: string; episode: string }>();
  const navigate = useNavigate();
  const slotRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  usePlayerHost(slotRef, setError);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const id = Number(watchId);
      const number = Number(episode);
      const [watches, episodes] = await Promise.all([listWatches(), listWatchEpisodes(id)]);
      const watch = watches.find((w) => w.id === id);
      const ep = episodes.find((e) => episodeNumberOf(e) === number);
      const source = ep && isPlayable(ep) ? (ep.item_path ?? ep.save_path) : null;
      if (!watch || !ep || !source) {
        setError("Esse episódio não está disponível pra assistir.");
        return;
      }
      if (cancelled) return;
      const rawLabel = parseEpisodeLabel(ep.name, ep.episode_number);
      const { title, episodeLabel } = formatPlayerTitle(watch.title, rawLabel, watch.series_title);
      const resume = !ep.watched_at && (ep.watch_position_ms ?? 0) > MIN_RESUME_MS ? ep.watch_position_ms : null;
      await playerOpen(source, title, episodeLabel, watch.id, number, resume);
    })().catch((e) => setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [watchId, episode]);

  // Saiu do player = para de tocar (senão o áudio seguia com a página fechada).
  useEffect(() => {
    return () => {
      playerStop().catch(() => {});
    };
  }, []);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black">
      <div ref={slotRef} className="absolute inset-0" />
      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 text-center">
          <p className="text-sm text-white/80">{error}</p>
          <button
            type="button"
            onClick={() => navigate(-1)}
            className="flex items-center gap-2 rounded-[10px] border border-white/20 px-4 py-2 text-[13px] font-semibold text-white transition-colors hover:bg-white/10"
          >
            <ArrowLeft className="size-4" />
            Voltar
          </button>
        </div>
      )}
    </div>
  );
}
