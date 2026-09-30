import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Loader2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { playerOpen, playerStop } from "@/lib/player";
import { episodeStreamStart, listWatchEpisodes } from "@/lib/episodes";
import { listWatches } from "@/lib/watches";
import { formatPlayerTitle, parseEpisodeLabel } from "@/lib/episodeName";
import { episodeNumberOf, isPlayable, isStreamable } from "@/lib/continueWatching";
import { usePlayerHost } from "@/lib/usePlayerHost";
import { getSettings } from "@/lib/tauri";
import { AmbientLight } from "@/components/player/AmbientLight";
import { ambientFromSettings, onAmbientChanged, type AmbientSettings } from "@/lib/ambient";

const MIN_RESUME_MS = 5_000;

export default function Player() {
  const { t } = useTranslation();
  const { watchId, episode } = useParams<{ watchId: string; episode: string }>();
  const navigate = useNavigate();
  const slotRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [preparing, setPreparing] = useState(false);
  const videoAspectRef = useRef<number | null>(null);
  const [ambient, setAmbient] = useState<AmbientSettings | null>(null);
  usePlayerHost(slotRef, setError, videoAspectRef);

  useEffect(() => {
    getSettings()
      .then((s) => setAmbient(ambientFromSettings(s)))
      .catch(() => {});
    const unlisten = onAmbientChanged(setAmbient);
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const id = Number(watchId);
      const number = Number(episode);
      const [watches, episodes] = await Promise.all([listWatches(), listWatchEpisodes(id)]);
      const watch = watches.find((w) => w.id === id);
      const ep = episodes.find((e) => episodeNumberOf(e) === number);
      let source: string | null = null;
      if (watch && ep && isPlayable(ep)) {
        source = ep.item_path;
      } else if (watch && ep && (isStreamable(ep) || watch.streaming)) {
        setPreparing(true);
        try {
          source = await episodeStreamStart(ep.id);
        } catch (e) {
          if (!cancelled) setError(String(e));
          return;
        } finally {
          if (!cancelled) setPreparing(false);
        }
      }
      if (!watch || !ep || !source) {
        setError(t("player.unavailable"));
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

  useEffect(() => {
    const unlisten = listen("app:window-hidden", () => navigate("/library", { replace: true }));
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [navigate]);

  useEffect(() => {
    return () => {
      playerStop().catch(() => {});
    };
  }, []);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black">
      {ambient?.enabled && <AmbientLight slotRef={slotRef} aspectRef={videoAspectRef} settings={ambient} />}
      <div ref={slotRef} className="absolute inset-0" />
      {preparing && !error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 text-center">
          <Loader2 className="size-10 animate-spin text-white/80" />
          <p className="text-sm text-white/80">{t("player.preparingStream")}</p>
          <button
            type="button"
            onClick={() => navigate(-1)}
            className="flex items-center gap-2 rounded-[10px] border border-white/20 px-4 py-2 text-[13px] font-semibold text-white transition-colors hover:bg-white/10"
          >
            <ArrowLeft className="size-4" />
            {t("common.back")}
          </button>
        </div>
      )}
      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 text-center">
          <p className="text-sm text-white/80">{error}</p>
          <button
            type="button"
            onClick={() => navigate(-1)}
            className="flex items-center gap-2 rounded-[10px] border border-white/20 px-4 py-2 text-[13px] font-semibold text-white transition-colors hover:bg-white/10"
          >
            <ArrowLeft className="size-4" />
            {t("common.back")}
          </button>
        </div>
      )}
    </div>
  );
}
