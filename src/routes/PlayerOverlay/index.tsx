import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { emit, listen } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  Captions,
  Check,
  ChevronDown,
  ListVideo,
  Maximize,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  SkipForward,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  mediaFrame,
  mediaProbe,
  playerBringToFront,
  playerGetSkipSegments,
  playerSaveProgress,
  playerListAudioTracks,
  playerListSubtitleTracks,
  playerOpen,
  playerSeek,
  playerSeekRelative,
  playerSetAudioTrack,
  playerSetPaused,
  playerSetSubtitleTrack,
  playerSubtitleCues,
  type SubtitleCue,
  playerSetVolume,
  playerSnapshot,
  type MediaProbe,
  type PlayerSnapshot,
  type ProbeTrack,
  type SkipSegments,
  type TrackInfo,
} from "@/lib/player";
import { episodePrefetchNext, episodeStreamStart, listWatchEpisodes, type Episode } from "@/lib/episodes";
import { isStreamStartable } from "@/lib/continueWatching";
import { formatPlayerTitle, parseEpisodeLabel, parseEpisodeNumber } from "@/lib/episodeName";
import { listWatches, type Watch } from "@/lib/watches";
import { getSettings } from "@/lib/tauri";
import { PlayerSettingsButton } from "@/components/player/PlayerSettingsButton";
import { SubtitleLayer } from "@/components/player/SubtitleLayer";
import {
  DEFAULT_SUBTITLE_STYLE,
  subtitleStyleFromSettings,
  type SubtitleStyle,
} from "@/lib/subtitleStyle";
import { episodeStatusLabel } from "@/lib/constants";
import { useTranslation } from "react-i18next";
import {
  findPreferredTrack,
  humanizeTrackLanguage,
  isOriginalTrack,
  probeTrackName,
  trackMatchesPreferred,
  trackQualifierTag,
} from "@/lib/playerLanguage";

const IDLE_HIDE_MS = 3000;
const SKIP_MS = 10_000;
const SEEK_SETTLE_MS = 1500;

function formatTime(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mm = hours > 0 ? String(minutes).padStart(2, "0") : String(minutes);
  return hours > 0 ? `${hours}:${mm}:${String(seconds).padStart(2, "0")}` : `${mm}:${String(seconds).padStart(2, "0")}`;
}

type SegmentKind = "intro" | "ending" | "recap";

const SEGMENT_KINDS: SegmentKind[] = ["recap", "intro", "ending"];

type SkipSettings = {
  auto: Record<SegmentKind, boolean>;
  mark: Record<SegmentKind, boolean>;
  nextAfterEnding: boolean;
};

const DEFAULT_SKIP_SETTINGS: SkipSettings = {
  auto: { intro: false, ending: false, recap: false },
  mark: { intro: true, ending: true, recap: true },
  nextAfterEnding: false,
};

const NEXT_AFTER_ENDING_MAX_TAIL_MS = 150_000;
const PROGRESS_SAVE_MS = 10_000;

function isMixed(s: SkipSegments, kind: SegmentKind): boolean {
  return (kind === "intro" && s.intro_mixed) || (kind === "ending" && s.ending_mixed);
}

function segmentRange(s: SkipSegments, kind: SegmentKind): [number, number] | null {
  const start = s[`${kind}_start_ms`];
  const end = s[`${kind}_end_ms`];
  return start != null && end != null && end > start ? [start, end] : null;
}

function isStreamSource(source: string | null | undefined): boolean {
  return !!source && source.startsWith("http://127.0.0.1:");
}

function episodeSource(ep: Episode): string | null {
  return ep.status === "available" ? (ep.item_path ?? null) : null;
}

function canPlay(ep: Episode, watch: Watch | null): boolean {
  return episodeSource(ep) != null || ep.status === "downloading" || isStreamStartable(watch, ep);
}

function episodeNumberOf(ep: Episode): number | null {
  return ep.episode_number ?? parseEpisodeNumber(ep.name);
}

async function openEpisode(ep: Episode, watchId: number, watch: Watch | null): Promise<void> {
  const source =
    episodeSource(ep) ??
    (ep.status === "downloading" || isStreamStartable(watch, ep) ? await episodeStreamStart(ep.id).catch(() => null) : null);
  if (!source) return;
  const rawLabel = parseEpisodeLabel(ep.name, ep.episode_number);
  const { title, episodeLabel } = watch
    ? formatPlayerTitle(watch.title, rawLabel, watch.series_title)
    : { title: "", episodeLabel: rawLabel };
  return playerOpen(source, title, episodeLabel, watchId, episodeNumberOf(ep));
}

const END_OF_FILE_SLACK_MS = 2_000;

function skipSegment(snap: PlayerSnapshot, endMs: number, seek: (ms: number) => void) {
  if (snap.duration_ms > 0 && endMs >= snap.duration_ms - END_OF_FILE_SLACK_MS) {
    openNextEpisode(snap)
      .then((opened) => {
        if (!opened) emit("player:back-requested");
      })
      .catch(() => {});
    return;
  }
  seek(endMs);
}

async function openNextEpisode(snap: PlayerSnapshot): Promise<boolean> {
  if (snap.watch_id == null || snap.episode_number == null) return false;
  const [episodes, watches] = await Promise.all([listWatchEpisodes(snap.watch_id), listWatches()]);
  const watch = watches.find((w) => w.id === snap.watch_id) ?? null;
  const next = episodes.find((e) => episodeNumberOf(e) === snap.episode_number! + 1 && canPlay(e, watch));
  if (!next) return false;
  await openEpisode(next, snap.watch_id, watch);
  return true;
}

export default function PlayerOverlay() {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<PlayerSnapshot | null>(null);
  const [stalled, setStalled] = useState(false);
  const lastMoveRef = useRef<{ pos: number; at: number }>({ pos: -1, at: 0 });
  const prefetchDoneRef = useRef(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [scrubbingPct, setScrubbingPct] = useState<number | null>(null);
  const [pendingSeek, setPendingSeek] = useState<{ ms: number; at: number } | null>(null);
  const [volumeHover, setVolumeHover] = useState(false);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const volumeReleaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const shortcutRef = useRef<(key: string, shift: boolean) => boolean>(() => false);
  const showControlsRef = useRef<() => void>(() => {});
  const mutedVolumeRef = useRef(80);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (shortcutRef.current(e.key, e.shiftKey)) e.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    const unlistenKey = listen<{ key: string; shift: boolean }>("player:shortcut", (event) =>
      shortcutRef.current(event.payload.key, event.payload.shift),
    );
    const unlistenNext = listen("player:next-episode-requested", () => shortcutRef.current("n", true));
    return () => {
      window.removeEventListener("keydown", onKey);
      unlistenKey.then((fn) => fn());
      unlistenNext.then((fn) => fn());
    };
  }, []);
  const [openMenu, setOpenMenu] = useState<"tracks" | "episodes" | "settings" | null>(null);
  const episodesOpen = openMenu === "episodes";
  const closingPanelAtRef = useRef(0);
  const [preferredLangs, setPreferredLangs] = useState<{ audio: string[]; subtitle: string[] }>({
    audio: [],
    subtitle: [],
  });
  const [skipSettings, setSkipSettings] = useState<SkipSettings>(DEFAULT_SKIP_SETTINGS);
  const [skipSegments, setSkipSegments] = useState<SkipSegments | null>(null);
  const idleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sessionKeyRef = useRef<string | null>(null);
  const autoSelectDoneRef = useRef(false);
  const skipFetchDoneRef = useRef(false);
  const autoSkippedRef = useRef<Record<SegmentKind, boolean>>({ intro: false, ending: false, recap: false });
  const nextTriggeredRef = useRef(false);
  const lastProgressSaveRef = useRef(0);
  const watchedMarkedRef = useRef(false);
  const [subStyle, setSubStyle] = useState<SubtitleStyle>(DEFAULT_SUBTITLE_STYLE);
  const [customOrdinal, setCustomOrdinal] = useState<number | null>(null);
  const customOrdinalRef = useRef<number | null>(null);
  customOrdinalRef.current = customOrdinal;
  const [cues, setCues] = useState<SubtitleCue[] | null>(null);
  const subResolvedRef = useRef(false);
  const customFailedRef = useRef(false);
  const autoSelectAtRef = useRef(0);

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    document.body.style.margin = "0";
    document.body.style.overflow = "hidden";
  }, []);

  useEffect(() => {
    const id = setInterval(() => {
      playerSnapshot()
        .then((snap) => {
          setSnapshot(snap);
          setPendingSeek((pending) => {
            if (!pending) return null;
            const settled = Math.abs(snap.position_ms - pending.ms) < 800;
            const timedOut = Date.now() - pending.at > SEEK_SETTLE_MS;
            return settled || timedOut ? null : pending;
          });

          const sessionKey = String(snap.session);
          if (sessionKeyRef.current !== sessionKey) {
            sessionKeyRef.current = sessionKey;
            autoSelectDoneRef.current = false;
            skipFetchDoneRef.current = false;
            prefetchDoneRef.current = false;
            autoSkippedRef.current = { intro: false, ending: false, recap: false };
            nextTriggeredRef.current = false;
            watchedMarkedRef.current = false;
            lastProgressSaveRef.current = 0;
            setSkipSegments(null);
            setCustomOrdinal(null);
            setCues(null);
            subResolvedRef.current = false;
            customFailedRef.current = false;
            setScrubbingPct(null);
            setPendingSeek(null);
            getSettings()
              .then((s) => {
                setPreferredLangs({
                  audio: s.player_preferred_audio_langs ? s.player_preferred_audio_langs.split(",") : [],
                  subtitle: s.player_preferred_subtitle_langs ? s.player_preferred_subtitle_langs.split(",") : [],
                });
                setSkipSettings({
                  auto: {
                    intro: s.player_auto_skip_intro === "1",
                    ending: s.player_auto_skip_ending === "1",
                    recap: s.player_auto_skip_recap === "1",
                  },
                  mark: {
                    intro: s.player_mark_intro !== "0",
                    ending: s.player_mark_ending !== "0",
                    recap: s.player_mark_recap !== "0",
                  },
                  nextAfterEnding: s.player_next_after_ending === "1",
                });
                setSubStyle(subtitleStyleFromSettings(s));
              })
              .catch(() => {});
          }
          if (!skipFetchDoneRef.current && snap.watch_id != null && snap.episode_number != null) {
            skipFetchDoneRef.current = true;
            playerGetSkipSegments(snap.watch_id, snap.episode_number)
              .then(setSkipSegments)
              .catch(() => {});
          }
          if (
            !prefetchDoneRef.current &&
            snap.watch_id != null &&
            snap.episode_number != null &&
            snap.duration_ms > 0 &&
            snap.position_ms >= snap.duration_ms / 2
          ) {
            prefetchDoneRef.current = true;
            episodePrefetchNext(snap.watch_id, snap.episode_number).catch(() => {});
          }
          if (snap.watch_id != null && snap.episode_number != null && snap.duration_ms > 0) {
            const endingStart = skipSegments ? segmentRange(skipSegments, "ending")?.[0] : undefined;
            const watchedPoint = endingStart ?? snap.duration_ms * 0.9;
            const now = Date.now();
            if (!watchedMarkedRef.current && (snap.position_ms >= watchedPoint || snap.state === "ended")) {
              watchedMarkedRef.current = true;
              lastProgressSaveRef.current = now;
              playerSaveProgress(snap.watch_id, snap.episode_number, snap.position_ms, true).catch(() => {});
            } else if (snap.is_playing && now - lastProgressSaveRef.current > PROGRESS_SAVE_MS) {
              lastProgressSaveRef.current = now;
              playerSaveProgress(snap.watch_id, snap.episode_number, snap.position_ms, false).catch(() => {});
            }
          }

          if (skipSegments) {
            for (const kind of SEGMENT_KINDS) {
              const range = segmentRange(skipSegments, kind);
              if (
                range &&
                skipSettings.auto[kind] &&
                !isMixed(skipSegments, kind) &&
                !autoSkippedRef.current[kind] &&
                snap.position_ms >= range[0] &&
                snap.position_ms < range[1]
              ) {
                autoSkippedRef.current[kind] = true;
                skipSegment(snap, range[1], (ms) => playerSeek(ms).catch(() => {}));
              }
            }

            const ending = segmentRange(skipSegments, "ending");
            if (
              skipSettings.nextAfterEnding &&
              ending &&
              !skipSegments.ending_mixed &&
              !nextTriggeredRef.current &&
              snap.duration_ms > 0
            ) {
              const tail = snap.duration_ms - ending[1];
              if (tail > 0 && tail <= NEXT_AFTER_ENDING_MAX_TAIL_MS && snap.position_ms >= ending[1] - 300) {
                nextTriggeredRef.current = true;
                openNextEpisode(snap).catch(() => {});
              }
            }
          }
          if (!autoSelectDoneRef.current && (preferredLangs.audio.length > 0 || preferredLangs.subtitle.length > 0)) {
            Promise.all([playerListAudioTracks(), playerListSubtitleTracks()])
              .then(([audioTracks, subtitleTracks]) => {
                if (audioTracks.length === 0 && subtitleTracks.length === 0) return;
                autoSelectDoneRef.current = true;
                autoSelectAtRef.current = Date.now();
                if (preferredLangs.audio.length > 0) {
                  const match = findPreferredTrack(audioTracks, preferredLangs.audio);
                  if (match) playerSetAudioTrack(match.id).catch(() => {});
                }
                if (preferredLangs.subtitle.length > 0) {
                  const match = findPreferredTrack(subtitleTracks, preferredLangs.subtitle);
                  if (match) playerSetSubtitleTrack(match.id).catch(() => {});
                }
              })
              .catch(() => {});
          }
          const noPrefs = preferredLangs.audio.length === 0 && preferredLangs.subtitle.length === 0;
          const prefsApplied = noPrefs || (autoSelectDoneRef.current && Date.now() - autoSelectAtRef.current > 600);
          if (
            subStyle.mode === "custom" &&
            !isStreamSource(snap.source) &&
            !subResolvedRef.current &&
            !customFailedRef.current &&
            prefsApplied
          ) {
            playerListSubtitleTracks()
              .then((tracks) => {
                if (tracks.length === 0) return;
                subResolvedRef.current = true;
                const active = tracks.filter((tr) => tr.id >= 0).findIndex((tr) => tr.active);
                if (active >= 0) {
                  setCustomOrdinal(active);
                  playerSetSubtitleTrack(-1).catch(() => {});
                }
              })
              .catch(() => {});
          }
        })
        .catch(() => {});
      playerBringToFront().catch(() => {});
    }, 400);
    return () => clearInterval(id);
  }, [preferredLangs, skipSettings, skipSegments, subStyle.mode]);

  const currentSource = snapshot?.source ?? null;
  useEffect(() => {
    if (subStyle.mode !== "custom" || customOrdinal == null || !currentSource || isStreamSource(currentSource)) {
      setCues(null);
      return;
    }
    let cancelled = false;
    playerSubtitleCues(currentSource, customOrdinal)
      .then((c) => {
        if (!cancelled) setCues(c);
      })
      .catch(() => {
        if (cancelled) return;
        customFailedRef.current = true;
        restoreVlcSubtitle(customOrdinal);
        setCustomOrdinal(null);
      });
    return () => {
      cancelled = true;
    };
  }, [currentSource, customOrdinal, subStyle.mode]);

  function restoreVlcSubtitle(ordinal: number) {
    playerListSubtitleTracks()
      .then((tracks) => {
        const track = tracks.filter((tr) => tr.id >= 0)[ordinal];
        if (track) playerSetSubtitleTrack(track.id).catch(() => {});
      })
      .catch(() => {});
  }

  function changeSubStyle(next: SubtitleStyle) {
    if (next.mode !== subStyle.mode) {
      if (next.mode === "custom") {
        subResolvedRef.current = false;
        customFailedRef.current = false;
      } else if (customOrdinalRef.current != null) {
        restoreVlcSubtitle(customOrdinalRef.current);
        setCustomOrdinal(null);
        setCues(null);
      }
    }
    setSubStyle(next);
  }

  function selectCustomSubtitle(ordinal: number | null) {
    subResolvedRef.current = true;
    customFailedRef.current = false;
    setCustomOrdinal(ordinal);
    playerSetSubtitleTrack(-1).catch(() => {});
  }

  useEffect(() => {
    function resetIdle() {
      setControlsVisible(true);
      if (idleTimer.current) clearTimeout(idleTimer.current);
      idleTimer.current = setTimeout(() => setControlsVisible(false), IDLE_HIDE_MS);
    }
    showControlsRef.current = resetIdle;
    resetIdle();
    window.addEventListener("mousemove", resetIdle);
    window.addEventListener("mousedown", resetIdle);
    return () => {
      window.removeEventListener("mousemove", resetIdle);
      window.removeEventListener("mousedown", resetIdle);
      if (idleTimer.current) clearTimeout(idleTimer.current);
    };
  }, []);

  const isPaused = snapshot?.state === "paused";

  useEffect(() => {
    if (!snapshot) return;
    const now = Date.now();
    const last = lastMoveRef.current;
    if (snapshot.position_ms !== last.pos) {
      lastMoveRef.current = { pos: snapshot.position_ms, at: now };
      if (snapshot.state !== "opening" && snapshot.state !== "buffering") {
        setStalled(false);
        return;
      }
    }
    const waiting =
      snapshot.state === "opening" ||
      snapshot.state === "buffering" ||
      (snapshot.state === "playing" && now - last.at > 1000);
    setStalled(waiting);
  }, [snapshot]);
  const durationMs = snapshot?.duration_ms ?? 0;
  const positionMs =
    scrubbingPct != null
      ? (scrubbingPct / 100) * durationMs
      : (pendingSeek?.ms ?? snapshot?.position_ms ?? 0);
  const volume = volumeDraft ?? snapshot?.volume ?? 80;

  function changeVolume(v: number) {
    if (volumeReleaseTimer.current) clearTimeout(volumeReleaseTimer.current);
    setVolumeDraft(v);
    playerSetVolume(v).catch(() => {});
  }

  function releaseVolume() {
    if (volumeReleaseTimer.current) clearTimeout(volumeReleaseTimer.current);
    volumeReleaseTimer.current = setTimeout(() => setVolumeDraft(null), 1000);
  }

  const segmentMarks: SeekSegment[] = [];
  let activeSkip: { label: string; endMs: number } | null = null;
  if (skipSegments && durationMs > 0) {
    const toPct = (ms: number) => Math.min(100, Math.max(0, (ms / durationMs) * 100));
    for (const kind of SEGMENT_KINDS) {
      const range = segmentRange(skipSegments, kind);
      if (!range) continue;
      if (skipSettings.mark[kind]) {
        segmentMarks.push({ startPct: toPct(range[0]), endPct: toPct(range[1]), label: t(`player.segment.${kind}`) });
      }
      const autoSkips = skipSettings.auto[kind] && !isMixed(skipSegments, kind);
      if (!autoSkips && positionMs >= range[0] && positionMs < range[1]) {
        activeSkip = { label: t(`player.skip.${kind}`), endMs: range[1] };
      }
    }
  }

  function togglePause() {
    playerSetPaused(!isPaused).catch(() => {});
  }

  function seekTo(ms: number) {
    const clamped = Math.max(0, Math.min(durationMs, Math.round(ms)));
    playerSeek(clamped).catch(() => {});
    setPendingSeek({ ms: clamped, at: Date.now() });
  }

  function handleShortcut(key: string, shift: boolean): boolean {
    if (!snapshot) return false;
    const k = key.length === 1 ? key.toLowerCase() : key;
    if (k === " " || k === "k") togglePause();
    else if (k === "j") seekTo(positionMs - 10_000);
    else if (k === "l") seekTo(positionMs + 10_000);
    else if (k === "ArrowLeft") seekTo(positionMs - 5_000);
    else if (k === "ArrowRight") seekTo(positionMs + 5_000);
    else if (k === "ArrowUp" || k === "ArrowDown") {
      changeVolume(Math.max(0, Math.min(100, volume + (k === "ArrowUp" ? 5 : -5))));
      releaseVolume();
    } else if (k === "m") {
      if (volume > 0) {
        mutedVolumeRef.current = volume;
        changeVolume(0);
      } else changeVolume(mutedVolumeRef.current || 80);
      releaseVolume();
    } else if (k === "f") emit("player:toggle-fullscreen-requested");
    else if (k === "Escape") emit("player:exit-fullscreen-requested");
    else if (shift && k === "n") openNextEpisode(snapshot).catch(() => {});
    else if (/^[0-9]$/.test(k) && durationMs > 0) seekTo((Number(k) / 10) * durationMs);
    else return false;
    showControlsRef.current();
    return true;
  }
  shortcutRef.current = handleShortcut;

  return (
    <div
      className="relative h-screen w-screen overflow-hidden"
      onPointerDownCapture={(e) => {
        const target = e.target as HTMLElement;
        if (openMenu === "episodes" && !target.closest("[data-episodes-panel], [data-episodes-toggle]")) {
          closingPanelAtRef.current = Date.now();
          setOpenMenu(null);
        }
      }}
      onClick={() => {
        if (Date.now() - closingPanelAtRef.current < 600) return;
        togglePause();
      }}
    >
      {subStyle.mode === "custom" && cues && snapshot && (
        <SubtitleLayer
          cues={cues}
          positionMs={snapshot.position_ms}
          playing={snapshot.is_playing}
          style={subStyle}
          controlsVisible={controlsVisible}
        />
      )}

      {stalled && !controlsVisible && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <span className="size-[76px] animate-spin rounded-full border-[3px] border-white/20 border-t-white" />
        </div>
      )}
      {stalled && isStreamSource(snapshot?.source) && (
        <div className="pointer-events-none absolute inset-x-0 top-1/2 flex translate-y-16 justify-center">
          <span className="rounded-md bg-black/60 px-3 py-1 text-[13px] font-medium text-white/90">
            {t("player.streamBuffering")}
          </span>
        </div>
      )}

      <div
        className="absolute inset-x-0 top-0 flex items-center gap-4 px-6 pt-5 pb-16 transition-opacity duration-300"
        style={{
          opacity: controlsVisible ? 1 : 0,
          background: "linear-gradient(to bottom, rgba(0,0,0,0.75) 0%, rgba(0,0,0,0.28) 65%, rgba(0,0,0,0) 100%)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          aria-label={t("common.back")}
          onClick={() => emit("player:back-requested")}
          className="flex size-9 shrink-0 items-center justify-center rounded-full text-white transition-colors hover:bg-white/15"
        >
          <ArrowLeft className="size-5" />
        </button>
        <div className="flex min-w-0 flex-col gap-0.5">
          {snapshot?.title && <h1 className="truncate text-xl font-bold text-white">{snapshot.title}</h1>}
          {snapshot?.episode_label && <EpisodeHeading label={snapshot.episode_label} />}
        </div>
      </div>

      <div
        className="pointer-events-none absolute inset-0 flex items-center justify-center transition-opacity duration-300"
        style={{ opacity: controlsVisible ? 1 : 0 }}
      >
        <div
          className="pointer-events-auto flex items-center gap-10"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            aria-label={t("player.back10")}
            onClick={() => playerSeekRelative(-SKIP_MS).catch(() => {})}
            className="relative flex size-12 items-center justify-center text-white/90 transition-colors hover:text-white"
          >
            <RotateCcw className="size-8" strokeWidth={1.6} />
            <span className="absolute text-[10px] font-bold">10</span>
          </button>
          <button
            type="button"
            aria-label={isPaused ? t("player.play") : t("player.pause")}
            onClick={togglePause}
            className="relative flex size-16 items-center justify-center rounded-full bg-white/15 text-white backdrop-blur-sm transition-colors hover:bg-white/25"
          >
            {stalled && !isPaused && (
              <span className="pointer-events-none absolute -inset-1.5 animate-spin rounded-full border-[3px] border-white/20 border-t-white" />
            )}
            {isPaused ? (
              <Play className="size-7 translate-x-0.5" fill="currentColor" />
            ) : (
              <Pause className="size-7" fill="currentColor" />
            )}
          </button>
          <button
            type="button"
            aria-label={t("player.forward10")}
            onClick={() => playerSeekRelative(SKIP_MS).catch(() => {})}
            className="relative flex size-12 items-center justify-center text-white/90 transition-colors hover:text-white"
          >
            <RotateCw className="size-8" strokeWidth={1.6} />
            <span className="absolute text-[10px] font-bold">10</span>
          </button>
        </div>
      </div>

      {activeSkip && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            if (snapshot) skipSegment(snapshot, activeSkip.endMs, seekTo);
          }}
          className="absolute right-6 bottom-24 z-10 rounded-lg border border-white/20 bg-black/60 px-4 py-2.5 text-sm font-semibold text-white backdrop-blur-sm transition-colors hover:bg-black/80"
        >
          {activeSkip.label}
        </button>
      )}

      <div
        className="absolute inset-x-0 bottom-0 flex flex-col gap-2 px-6 pt-16 pb-5 transition-opacity duration-300"
        style={{
          opacity: controlsVisible ? 1 : 0,
          background: "linear-gradient(to top, rgba(0,0,0,0.88) 0%, rgba(0,0,0,0.4) 50%, rgba(0,0,0,0) 100%)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <SeekBar
          durationMs={durationMs}
          positionMs={positionMs}
          segments={segmentMarks}
          source={snapshot?.source || null}
          onScrub={setScrubbingPct}
          onSeek={seekTo}
        />

        <div className="flex items-center gap-4">
          <span className="text-xs font-medium tabular-nums text-white/85">
            {formatTime(positionMs)} / {formatTime(durationMs)}
          </span>

          <div className="ml-auto flex items-center gap-1">
            <TrackPickerButton
              preferredAudio={preferredLangs.audio}
              preferredSubtitle={preferredLangs.subtitle}
              open={openMenu === "tracks"}
              setOpen={(o) => setOpenMenu((m) => (o ? "tracks" : m === "tracks" ? null : m))}
              customSubtitle={subStyle.mode === "custom" && !customFailedRef.current ? customOrdinal : undefined}
              onCustomSubtitle={selectCustomSubtitle}
            />
            <PlayerSettingsButton
              open={openMenu === "settings"}
              setOpen={(o) => setOpenMenu((m) => (o ? "settings" : m === "settings" ? null : m))}
              subStyle={subStyle}
              onSubStyleChange={changeSubStyle}
            />

            <button
              type="button"
              aria-label={t("detail.episodes")}
              data-episodes-toggle
              onClick={() => setOpenMenu((m) => (m === "episodes" ? null : "episodes"))}
              className={`flex size-8 items-center justify-center rounded-full transition-colors ${
                episodesOpen ? "bg-white/20 text-white" : "text-white hover:bg-white/10"
              }`}
            >
              <ListVideo className="size-4" />
            </button>

            <button
              type="button"
              aria-label={t("player.nextEpisode")}
              onClick={() => {
                if (snapshot) openNextEpisode(snapshot).catch(() => {});
              }}
              className="flex size-8 items-center justify-center rounded-full text-white transition-colors hover:bg-white/10"
            >
              <SkipForward className="size-4" fill="currentColor" />
            </button>

            <div
              className="flex items-center gap-2"
              onMouseEnter={() => setVolumeHover(true)}
              onMouseLeave={() => setVolumeHover(false)}
            >
              <button
                type="button"
                aria-label={volume > 0 ? t("player.mute") : t("player.unmute")}
                onClick={() => {
                  changeVolume(volume > 0 ? 0 : 80);
                  releaseVolume();
                }}
                className="flex size-8 shrink-0 items-center justify-center rounded-full text-white transition-colors hover:bg-white/10"
              >
                {volume > 0 ? <Volume2 className="size-4" /> : <VolumeX className="size-4" />}
              </button>
              <input
                type="range"
                min={0}
                max={100}
                value={volume}
                onChange={(e) => changeVolume(Number(e.target.value))}
                onPointerUp={releaseVolume}
                className={`h-1 cursor-pointer accent-[#FF6A45] transition-all duration-200 ${
                  volumeHover ? "w-20 opacity-100" : "w-0 opacity-0"
                }`}
              />
            </div>

            <button
              type="button"
              aria-label={t("player.fullscreen")}
              onClick={() => emit("player:toggle-fullscreen-requested")}
              className="flex size-8 items-center justify-center rounded-full text-white transition-colors hover:bg-white/10"
            >
              <Maximize className="size-4" />
            </button>
          </div>
        </div>
      </div>

      <EpisodesPanel
        open={episodesOpen}
        watchId={snapshot?.watch_id ?? null}
        currentEpisode={snapshot?.episode_number ?? null}
        preferredAudio={preferredLangs.audio}
        preferredSubtitle={preferredLangs.subtitle}
        onClose={() => setOpenMenu((m) => (m === "episodes" ? null : m))}
      />
    </div>
  );
}

function EpisodeHeading({ label }: { label: string }) {
  const match = label.match(/^(.+?)\s+-\s+(?=Episódio|Episode)/);
  return (
    <h2 className="flex min-w-0 items-center gap-2 text-sm text-white/75">
      {match && (
        <span className="shrink-0 rounded-md border border-white/20 bg-white/10 px-1.5 py-0.5 text-[11px] font-semibold text-white">
          {match[1]}
        </span>
      )}
      <span className="truncate">{match ? label.slice(match[0].length) : label}</span>
    </h2>
  );
}

type SeekSegment ={ startPct: number; endPct: number; label: string };

const PREVIEW_STEP_MS = 5000;
const previewCache = new Map<string, string>();

function SeekBar({
  durationMs,
  positionMs,
  segments,
  source,
  onScrub,
  onSeek,
}: {
  durationMs: number;
  positionMs: number;
  segments: SeekSegment[];
  source: string | null;
  onScrub: (pct: number | null) => void;
  onSeek: (ms: number) => void;
}) {
  const barRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);
  const [hoverPct, setHoverPct] = useState<number | null>(null);
  const pct = durationMs > 0 ? Math.min(100, Math.max(0, (positionMs / durationMs) * 100)) : 0;

  const previewSource = source && !/^https?:\/\//.test(source) ? source : null;
  const [preview, setPreview] = useState<string | null>(null);
  const wantedRef = useRef<number | null>(null);
  const inflightRef = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverBucket =
    hoverPct != null && durationMs > 0
      ? Math.min(
          Math.max(0, durationMs - 1000),
          Math.round(((hoverPct / 100) * durationMs) / PREVIEW_STEP_MS) * PREVIEW_STEP_MS,
        )
      : null;

  function fetchLatest(src: string) {
    const bucket = wantedRef.current;
    if (inflightRef.current || bucket == null) return;
    const key = `${src}|${bucket}`;
    inflightRef.current = true;
    mediaFrame(src, bucket)
      .then((url) => {
        if (previewCache.size > 500) previewCache.clear();
        previewCache.set(key, url);
        if (wantedRef.current === bucket) setPreview(url);
      })
      .catch(() => {})
      .finally(() => {
        inflightRef.current = false;
        const next = wantedRef.current;
        if (next != null && next !== bucket) {
          const hit = previewCache.get(`${src}|${next}`);
          if (hit) setPreview(hit);
          else fetchLatest(src);
        }
      });
  }

  useEffect(() => {
    if (!previewSource || hoverBucket == null) {
      wantedRef.current = null;
      return;
    }
    wantedRef.current = hoverBucket;
    const hit = previewCache.get(`${previewSource}|${hoverBucket}`);
    if (hit) {
      setPreview(hit);
      return;
    }
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => fetchLatest(previewSource), 120);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [previewSource, hoverBucket]);

  useEffect(() => setPreview(null), [previewSource]);

  function pctAt(clientX: number): number {
    const rect = barRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    return Math.min(100, Math.max(0, ((clientX - rect.left) / rect.width) * 100));
  }

  const bounds = [...new Set([0, 100, ...segments.flatMap((s) => [s.startPct, s.endPct])])].sort((a, b) => a - b);
  const pieces = bounds
    .slice(0, -1)
    .map((start, i) => {
      const end = bounds[i + 1];
      const segment = segments.find((s) => start >= s.startPct && end <= s.endPct);
      return { start, end, segment };
    })
    .filter((p) => p.end - p.start > 0.05);

  const hoverSegment =
    hoverPct != null ? segments.find((s) => hoverPct >= s.startPct && hoverPct < s.endPct) : undefined;

  function endDrag(clientX: number | null) {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    onScrub(null);
    if (clientX != null) onSeek((pctAt(clientX) / 100) * durationMs);
  }

  return (
    <div
      ref={barRef}
      className="group relative flex h-5 w-full cursor-pointer items-center"
      onPointerDown={(e) => {
        if (durationMs <= 0) return;
        draggingRef.current = true;
        e.currentTarget.setPointerCapture(e.pointerId);
        onScrub(pctAt(e.clientX));
      }}
      onPointerMove={(e) => {
        const p = pctAt(e.clientX);
        setHoverPct(p);
        if (draggingRef.current) onScrub(p);
      }}
      onPointerUp={(e) => endDrag(e.clientX)}
      onPointerCancel={() => endDrag(null)}
      onLostPointerCapture={() => endDrag(null)}
      onPointerLeave={() => setHoverPct(null)}
    >
      <div className="flex h-1 w-full gap-[3px] transition-all duration-150 group-hover:h-1.5">
        {pieces.map((p) => {
          const fill = Math.min(100, Math.max(0, ((pct - p.start) / (p.end - p.start)) * 100));
          return (
            <div
              key={p.start}
              className={`relative h-full overflow-hidden rounded-full ${p.segment ? "bg-[#FFC24B]/55" : "bg-white/25"}`}
              style={{ flexGrow: p.end - p.start, flexBasis: 0 }}
            >
              <div className="absolute inset-y-0 left-0 bg-[#FF6A45]" style={{ width: `${fill}%` }} />
            </div>
          );
        })}
      </div>

      <div
        className="pointer-events-none absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-md transition-transform group-hover:scale-110"
        style={{ left: `${pct}%` }}
      />

      {hoverPct != null && durationMs > 0 && (
        <div
          className="pointer-events-none absolute bottom-full mb-2 flex -translate-x-1/2 flex-col items-center gap-1.5"
          style={{
            left: previewSource
              ? `clamp(100px, ${hoverPct}%, calc(100% - 100px))`
              : `clamp(40px, ${hoverPct}%, calc(100% - 40px))`,
          }}
        >
          {previewSource && (
            <div className="h-[108px] w-[192px] overflow-hidden rounded-lg border border-white/20 bg-black shadow-lg">
              {preview ? (
                <img src={preview} alt="" className="size-full object-cover" />
              ) : (
                <div className="size-full animate-pulse bg-white/5" />
              )}
            </div>
          )}
          <div className="rounded-md bg-black/85 px-2 py-1 text-[11px] font-semibold whitespace-nowrap text-white tabular-nums">
            {hoverSegment && <span className="mr-1.5 text-[#FFC24B]">{hoverSegment.label} ·</span>}
            {formatTime((hoverPct / 100) * durationMs)}
          </div>
        </div>
      )}
    </div>
  );
}

function qualityLabel(probe: MediaProbe | undefined, releaseName: string | null): string | null {
  if (probe && probe.width > 0) {
    if (probe.width >= 3800) return "4K";
    if (probe.width >= 2500) return "1440p";
    if (probe.width >= 1900) return "1080p";
    if (probe.width >= 1260) return "720p";
    return `${probe.height}p`;
  }
  const match = releaseName?.match(/\b(2160p|4k|1440p|1080p|720p|480p)\b/i);
  return match ? match[1].toLowerCase().replace("4k", "4K") : null;
}

function trackChips(tracks: ProbeTrack[], preferred: string[]): { label: string; original: boolean; tag: string | null }[] {
  const seen = new Set<string>();
  const out: { label: string; original: boolean; tag: string | null }[] = [];
  for (const t of tracks) {
    const name = probeTrackName(t.language, t.description);
    if (!trackMatchesPreferred(name, preferred)) continue;
    const label = humanizeTrackLanguage(name);
    const tag = trackQualifierTag(name);
    const key = `${label}|${tag ?? ""}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ label, original: isOriginalTrack(name), tag });
  }
  return out;
}

const probeCache = new Map<string, MediaProbe>();
const thumbCache = new Map<string, string>();

function EpisodeDetails({
  episode,
  preferredAudio,
  preferredSubtitle,
  onPlay,
}: {
  episode: Episode;
  preferredAudio: string[];
  preferredSubtitle: string[];
  onPlay: (() => void) | null;
}) {
  const { t } = useTranslation();
  const source = episodeSource(episode);
  const [probe, setProbe] = useState<MediaProbe | undefined>(source ? probeCache.get(source) : undefined);
  const [thumb, setThumb] = useState<string | undefined>(source ? thumbCache.get(source) : undefined);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!source) return;
    let cancelled = false;
    (async () => {
      try {
        let p = probeCache.get(source);
        if (!p) {
          p = await mediaProbe(source);
          probeCache.set(source, p);
        }
        if (cancelled) return;
        setProbe(p);
        if (!thumbCache.has(source) && p.duration_ms > 0) {
          const url = await mediaFrame(source, p.duration_ms * 0.3);
          thumbCache.set(source, url);
          if (!cancelled) setThumb(url);
        }
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [source]);

  const quality = qualityLabel(probe, episode.name);
  const audio = probe ? trackChips(probe.audio, preferredAudio) : [];
  const subtitles = probe ? trackChips(probe.subtitles, preferredSubtitle) : [];

  return (
    <div className="flex flex-col gap-3 px-3 pt-1 pb-3">
      {source && (
        <button
          type="button"
          disabled={!onPlay}
          onClick={onPlay ?? undefined}
          className="group/thumb relative aspect-video w-full overflow-hidden rounded-md bg-white/5"
        >
          {thumb ? (
            <img src={thumb} alt="" className="size-full object-cover" />
          ) : (
            !failed && <div className="size-full animate-pulse bg-white/5" />
          )}
          {onPlay && (
            <span className="absolute inset-0 flex items-center justify-center bg-black/25 opacity-0 transition-opacity group-hover/thumb:opacity-100">
              <span className="flex size-11 items-center justify-center rounded-full bg-white/90 text-black">
                <Play className="size-5 translate-x-0.5" fill="currentColor" />
              </span>
            </span>
          )}
        </button>
      )}

      <div className="flex flex-wrap items-center gap-1.5">
        <span className="rounded-md bg-white/10 px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white/80 uppercase">
          {episodeStatusLabel(episode.status)}
        </span>
        {quality && (
          <span className="rounded-md border border-white/20 px-1.5 py-0.5 text-[10px] font-bold text-white/80">
            {quality}
          </span>
        )}
      </div>

      {!source && onPlay ? (
        <button
          type="button"
          onClick={onPlay}
          className="flex items-center justify-center gap-2 rounded-md bg-white/90 px-3 py-2 text-[12px] font-semibold text-black transition-colors hover:bg-white"
        >
          <Play className="size-3.5" fill="currentColor" />
          {t("detail.watchNow")}
        </button>
      ) : !source ? (
        <p className="text-[11px] text-white/45">{t("player.notDownloadedYet")}</p>
      ) : failed ? (
        <p className="text-[11px] text-white/45">{t("player.readFailed")}</p>
      ) : (
        probe && (
          <>
            <ChipRow label={t("detail.audio")} chips={audio} />
            <ChipRow label={t("detail.subtitle")} chips={subtitles} />
          </>
        )
      )}
    </div>
  );
}

function ChipRow({ label, chips }: { label: string; chips: { label: string; original: boolean; tag: string | null }[] }) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[10px] font-bold tracking-wide text-white/45 uppercase">{label}</span>
      {chips.length === 0 ? (
        <span className="text-[11px] text-white/45">{t("player.none")}</span>
      ) : (
        <div className="flex flex-wrap gap-1">
          {chips.map((c) => (
            <span
              key={`${c.label}|${c.tag}`}
              className="flex items-center gap-1 rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/85"
            >
              {c.label}
              {c.original && <span className="text-[9px] font-bold text-primary uppercase">{t("player.original")}</span>}
              {c.tag && <span className="text-[9px] font-bold text-white/55 uppercase">{c.tag}</span>}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function EpisodesPanel({
  open,
  watchId,
  currentEpisode,
  preferredAudio,
  preferredSubtitle,
  onClose,
}: {
  open: boolean;
  watchId: number | null;
  currentEpisode: number | null;
  preferredAudio: string[];
  preferredSubtitle: string[];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [episodes, setEpisodes] = useState<Episode[]>([]);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const { data: watches = [] } = useQuery({
    queryKey: ["watches"],
    queryFn: listWatches,
    enabled: open && watchId != null,
  });
  const watch = watches.find((w) => w.id === watchId);

  useEffect(() => {
    if (!open || watchId == null) return;
    listWatchEpisodes(watchId)
      .then(setEpisodes)
      .catch(() => {});
  }, [open, watchId]);

  const numberOf = episodeNumberOf;
  const sorted = episodes
    .filter((e) => e.status !== "deleted")
    .slice()
    .sort((a, b) => (numberOf(a) ?? 0) - (numberOf(b) ?? 0));

  useEffect(() => {
    if (!open) return;
    const current = sorted.find((e) => currentEpisode != null && numberOf(e) === currentEpisode);
    if (current) setExpandedId(current.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, currentEpisode, episodes.length]);

  function play(ep: Episode) {
    if (watchId == null) return;
    openEpisode(ep, watchId, watch ?? null).catch(() => {});
    onClose();
  }

  return (
    <div
      data-episodes-panel
      className="absolute inset-y-0 right-0 flex w-[380px] flex-col border-l border-white/10 bg-[#0B0C10]/95 backdrop-blur-md transition-transform duration-300"
      style={{ transform: open ? "translateX(0)" : "translateX(100%)" }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="flex items-center justify-between border-b border-white/10 px-4 py-3.5">
        <span className="text-sm font-bold text-white">{t("detail.episodes")}</span>
        <button
          type="button"
          aria-label={t("common.close")}
          onClick={onClose}
          className="flex size-7 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white"
        >
          <X className="size-4" />
        </button>
      </div>
      <div className="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
        {watchId == null ? (
          <p className="px-2 py-4 text-center text-xs text-white/50">{t("player.noAnime")}</p>
        ) : sorted.length === 0 ? (
          <p className="px-2 py-4 text-center text-xs text-white/50">{t("player.noEpisodes")}</p>
        ) : (
          sorted.map((ep) => {
            const expanded = expandedId === ep.id;
            const isCurrent = currentEpisode != null && numberOf(ep) === currentEpisode;
            const available = canPlay(ep, watch ?? null);
            return (
              <div
                key={ep.id}
                className={`overflow-hidden rounded-lg transition-colors ${expanded ? "bg-white/[0.06]" : ""}`}
              >
                <button
                  type="button"
                  onClick={() => setExpandedId(expanded ? null : ep.id)}
                  className="flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-white/10"
                >
                  <span className={`flex min-w-0 items-center gap-2 text-xs font-medium ${available ? "text-white" : "text-white/55"}`}>
                    <span className="truncate">{parseEpisodeLabel(ep.name, ep.episode_number)}</span>
                    {isCurrent && (
                      <span className="shrink-0 rounded bg-primary/20 px-1 py-px text-[9px] font-bold text-primary uppercase">
                        {t("player.watching")}
                      </span>
                    )}
                  </span>
                  <span className="flex shrink-0 items-center gap-2">
                    {!expanded && (
                      <span className="text-[10px] font-bold tracking-wide text-white/50 uppercase">
                        {episodeStatusLabel(ep.status)}
                      </span>
                    )}
                    <ChevronDown
                      className={`size-4 text-white/60 transition-transform duration-200 ${expanded ? "rotate-180" : ""}`}
                    />
                  </span>
                </button>
                {expanded && (
                  <EpisodeDetails
                    episode={ep}
                    preferredAudio={preferredAudio}
                    preferredSubtitle={preferredSubtitle}
                    onPlay={available && !isCurrent ? () => play(ep) : null}
                  />
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

function TrackPickerButton({
  preferredAudio,
  preferredSubtitle,
  open,
  setOpen,
  customSubtitle,
  onCustomSubtitle,
}: {
  preferredAudio: string[];
  preferredSubtitle: string[];
  open: boolean;
  setOpen: (open: boolean) => void;
  customSubtitle?: number | null;
  onCustomSubtitle: (ordinal: number | null) => void;
}) {
  const { t } = useTranslation();
  const [audioTracks, setAudioTracks] = useState<TrackInfo[]>([]);
  const [subtitleTracks, setSubtitleTracks] = useState<TrackInfo[]>([]);

  useEffect(() => {
    if (!open) return;
    playerListAudioTracks()
      .then(setAudioTracks)
      .catch(() => {});
    playerListSubtitleTracks()
      .then(setSubtitleTracks)
      .catch(() => {});
  }, [open]);

  const filteredAudio = audioTracks.filter((track) => track.id >= 0 && trackMatchesPreferred(track.name, preferredAudio));
  const realSubtitles = subtitleTracks.filter((track) => track.id >= 0);
  const customMode = customSubtitle !== undefined;
  const filteredSubtitle = subtitleTracks
    .filter((track) => track.id < 0 || trackMatchesPreferred(track.name, preferredSubtitle))
    .map((track) =>
      customMode
        ? {
            ...track,
            active: track.id < 0 ? customSubtitle === null : realSubtitles.indexOf(track) === customSubtitle,
          }
        : track,
    );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t("player.audioAndSubtitles")}
          onClick={(e) => e.stopPropagation()}
          className="flex size-8 items-center justify-center rounded-full text-white transition-colors hover:bg-white/10"
        >
          <Captions className="size-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="top"
        onClick={(e) => e.stopPropagation()}
        className="max-h-[60vh] w-64 overflow-y-auto border-[#262A35] bg-[#15171D] p-1.5 text-foreground"
      >
        <TrackSection
          label={t("detail.audio")}
          tracks={filteredAudio}
          onSelect={(id) => playerSetAudioTrack(id).then(() => setOpen(false))}
        />
        <div className="my-1.5 h-px bg-[#1E212A]" />
        <TrackSection
          label={t("detail.subtitle")}
          tracks={filteredSubtitle}
          onSelect={(id) => {
            if (customMode) {
              onCustomSubtitle(id < 0 ? null : realSubtitles.findIndex((track) => track.id === id));
              setOpen(false);
            } else {
              playerSetSubtitleTrack(id).then(() => setOpen(false));
            }
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function TrackSection({
  label,
  tracks,
  onSelect,
}: {
  label: string;
  tracks: TrackInfo[];
  onSelect: (id: number) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-0.5">
      <span className="px-2 py-1 text-[10px] font-bold tracking-wide text-[#6C7180] uppercase">{label}</span>
      {tracks.length === 0 ? (
        <span className="px-2 py-1 text-xs text-[#6C7180]">{t("player.none")}</span>
      ) : (
        tracks.map((track) => {
          const tag = trackQualifierTag(track.name);
          const original = track.id >= 0 && isOriginalTrack(track.name);
          return (
            <button
              key={track.id}
              type="button"
              onClick={() => onSelect(track.id)}
              className={`flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors ${
                track.active ? "bg-secondary text-primary" : "hover:bg-white/5"
              }`}
            >
              <span className="flex min-w-0 items-center gap-1.5">
                <span className="truncate">{track.id < 0 ? t("player.disable") : humanizeTrackLanguage(track.name)}</span>
                {original && (
                  <span className="shrink-0 rounded-full bg-primary/20 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-primary uppercase">
                    {t("player.original")}
                  </span>
                )}
                {tag && (
                  <span className="shrink-0 rounded-full bg-white/10 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-white/70 uppercase">
                    {tag}
                  </span>
                )}
              </span>
              {track.active && <Check className="size-3.5 shrink-0" />}
            </button>
          );
        })
      )}
    </div>
  );
}
