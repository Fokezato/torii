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
  playerSetVolume,
  playerSnapshot,
  type MediaProbe,
  type PlayerSnapshot,
  type ProbeTrack,
  type SkipSegments,
  type TrackInfo,
} from "@/lib/player";
import { listWatchEpisodes, type Episode } from "@/lib/episodes";
import { formatPlayerTitle, parseEpisodeLabel, parseEpisodeNumber } from "@/lib/episodeName";
import { listWatches, type Watch } from "@/lib/watches";
import { getSettings } from "@/lib/tauri";
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
// Depois de um seek, ignora a posição que vem do poll até ela convergir
// pro alvo (ou esse tanto de tempo passar) — sem isso a agulha "voltava"
// pro valor antigo por até 1 tick de poll antes de avançar de novo (bug
// real reportado: pisca pra frente, volta, avança nde novo).
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

const SEGMENT_KINDS: { kind: SegmentKind; label: string; skipLabel: string }[] = [
  { kind: "recap", label: "Resumo", skipLabel: "Pular resumo" },
  { kind: "intro", label: "Abertura", skipLabel: "Pular abertura" },
  { kind: "ending", label: "Encerramento", skipLabel: "Pular encerramento" },
];

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

// Sobra depois do encerramento até isso = prévia/créditos, pula pro próximo
// episódio. Mais que isso costuma ser cena de história depois do ED.
const NEXT_AFTER_ENDING_MAX_TAIL_MS = 150_000;
const PROGRESS_SAVE_MS = 10_000;

function segmentRange(s: SkipSegments, kind: SegmentKind): [number, number] | null {
  const start = s[`${kind}_start_ms`];
  const end = s[`${kind}_end_ms`];
  return start != null && end != null && end > start ? [start, end] : null;
}

function episodeSource(ep: Episode): string | null {
  return ep.status === "available" ? (ep.item_path ?? ep.save_path ?? null) : null;
}

function episodeNumberOf(ep: Episode): number | null {
  return ep.episode_number ?? parseEpisodeNumber(ep.name);
}

/// Troca a mídia tocando pra esse episódio. `watch` dá o título CRU da
/// temporada e o nome do anime — `formatPlayerTitle` monta H1/H2.
function openEpisode(ep: Episode, watchId: number, watch: Watch | null): Promise<void> {
  const source = episodeSource(ep);
  if (!source) return Promise.resolve();
  const rawLabel = parseEpisodeLabel(ep.name, ep.episode_number);
  const { title, episodeLabel } = watch
    ? formatPlayerTitle(watch.title, rawLabel, watch.series_title)
    : { title: "", episodeLabel: rawLabel };
  return playerOpen(source, title, episodeLabel, watchId, episodeNumberOf(ep));
}

/// Próximo episódio JÁ BAIXADO do mesmo anime. `false` se não tiver.
async function openNextEpisode(snap: PlayerSnapshot): Promise<boolean> {
  if (snap.watch_id == null || snap.episode_number == null) return false;
  const [episodes, watches] = await Promise.all([listWatchEpisodes(snap.watch_id), listWatches()]);
  const next = episodes.find((e) => episodeNumberOf(e) === snap.episode_number! + 1 && episodeSource(e));
  if (!next) return false;
  const watch = watches.find((w) => w.id === snap.watch_id);
  await openEpisode(next, snap.watch_id, watch ?? null);
  return true;
}

const EPISODE_STATUS_LABEL: Record<string, string> = {
  pending: "Procurando",
  found: "Iniciando",
  downloading: "Baixando",
  available: "Pronto",
  error: "Erro",
};

/// Janela transparente por cima do vídeo (ver `spawn_player_overlay_window`
/// no lib.rs — `.owner()` mantém ela sempre acima da janela principal,
/// criada 1x no boot). Some sozinha com o mouse parado, igual Netflix/
/// YouTube; clicar em qualquer lugar (fora dos controles) alterna
/// play/pause. "Voltar" e "próximo episódio" só emitem evento — quem tem
/// o contexto de navegação (a página que abriu o player) escuta.
export default function PlayerOverlay() {
  const [snapshot, setSnapshot] = useState<PlayerSnapshot | null>(null);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [scrubbingPct, setScrubbingPct] = useState<number | null>(null);
  const [pendingSeek, setPendingSeek] = useState<{ ms: number; at: number } | null>(null);
  const [volumeHover, setVolumeHover] = useState(false);
  // Valor local do slider enquanto arrasta — sem isso ele seguia o volume
  // do poll (400ms) e dava "snap" pro valor antigo no meio do arraste.
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const volumeReleaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Atalhos de teclado: o listener é registrado 1x, mas chama sempre a
  // versão mais nova do tratador (que lê o estado atual).
  const shortcutRef = useRef<(key: string, shift: boolean) => boolean>(() => false);
  const showControlsRef = useRef<() => void>(() => {});
  const mutedVolumeRef = useRef(80);

  useEffect(() => {
    // Foco nos controles (essa janela): tecla chega aqui direto.
    function onKey(e: KeyboardEvent) {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (shortcutRef.current(e.key, e.shiftKey)) e.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    // Foco na janela principal (página do player): ela repassa por evento.
    const unlistenKey = listen<{ key: string; shift: boolean }>("player:shortcut", (event) =>
      shortcutRef.current(event.payload.key, event.payload.shift),
    );
    // Tecla "próxima" do teclado de mídia (controle de mídia do Windows).
    const unlistenNext = listen("player:next-episode-requested", () => shortcutRef.current("n", true));
    return () => {
      window.removeEventListener("keydown", onKey);
      unlistenKey.then((fn) => fn());
      unlistenNext.then((fn) => fn());
    };
  }, []);
  // Menu de áudio/legenda e painel de episódios são exclusivos — abrir um
  // fecha o outro (antes ficavam os 2 abertos um em cima do outro).
  const [openMenu, setOpenMenu] = useState<"tracks" | "episodes" | null>(null);
  const episodesOpen = openMenu === "episodes";
  const closingPanelAtRef = useRef(0);
  const [preferredLangs, setPreferredLangs] = useState<{ audio: string[]; subtitle: string[] }>({
    audio: [],
    subtitle: [],
  });
  const [skipSettings, setSkipSettings] = useState<SkipSettings>(DEFAULT_SKIP_SETTINGS);
  const [skipSegments, setSkipSegments] = useState<SkipSegments | null>(null);
  const idleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Chave da sessão de mídia atual (título+episódio) — quando muda, é
  // episódio novo, autoseleciona idioma preferido de novo; enquanto igual,
  // não mexe mais (respeita troca manual do usuário via o seletor).
  const sessionKeyRef = useRef<string | null>(null);
  const autoSelectDoneRef = useRef(false);
  // Evita chamar playerGetSkipSegments de novo a cada tick de poll (só 1x
  // por sessão) e evita o autoskip re-disparar seek em loop enquanto o
  // usuário ainda está dentro do MESMO trecho já pulado.
  const skipFetchDoneRef = useRef(false);
  const autoSkippedRef = useRef<Record<SegmentKind, boolean>>({ intro: false, ending: false, recap: false });
  // "Próximo episódio depois do encerramento" dispara 1x por episódio.
  const nextTriggeredRef = useRef(false);
  // Progresso salvo a cada PROGRESS_SAVE_MS; "assistido" marcado 1x por episódio.
  const lastProgressSaveRef = useRef(0);
  const watchedMarkedRef = useRef(false);

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

          // Por número de abertura, não pelo título: sair e voltar pro mesmo
          // episódio reabre o arquivo na faixa padrão, e precisa reaplicar
          // idioma/skip/etc. mesmo com título igual.
          const sessionKey = String(snap.session);
          if (sessionKeyRef.current !== sessionKey) {
            sessionKeyRef.current = sessionKey;
            autoSelectDoneRef.current = false;
            skipFetchDoneRef.current = false;
            autoSkippedRef.current = { intro: false, ending: false, recap: false };
            nextTriggeredRef.current = false;
            watchedMarkedRef.current = false;
            lastProgressSaveRef.current = 0;
            setSkipSegments(null);
            // Episódio novo: agulha volta a seguir o player do zero, sem
            // herdar arraste/seek pendente do episódio anterior.
            setScrubbingPct(null);
            setPendingSeek(null);
            // Relê a config a cada episódio novo, não só 1x no boot — essa
            // janela é permanente (criada 1x, nunca remonta, ver
            // spawn_player_overlay_window), então um fetch só no mount
            // nunca via mudança feita depois em Config > Reprodução (bug
            // real reportado: filtro de legenda simplesmente não aplicava).
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
              })
              .catch(() => {});
          }
          // Busca 1x por sessão (watch_id/episode_number só ficam disponíveis
          // depois que `player_open` termina — podem não vir ainda no 1º
          // tick logo após trocar de episódio, por isso checa a cada tick até
          // conseguir em vez de só na troca de sessão acima).
          if (!skipFetchDoneRef.current && snap.watch_id != null && snap.episode_number != null) {
            skipFetchDoneRef.current = true;
            playerGetSkipSegments(snap.watch_id, snap.episode_number)
              .then(setSkipSegments)
              .catch(() => {});
          }
          // Progresso + "assistido" (chegou no encerramento; sem dado de
          // encerramento, 90% do episódio). Base do "apagar depois de
          // assistir" — ver engine::cleanup_once no Rust.
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
            for (const { kind } of SEGMENT_KINDS) {
              const range = segmentRange(skipSegments, kind);
              if (
                range &&
                skipSettings.auto[kind] &&
                !autoSkippedRef.current[kind] &&
                snap.position_ms >= range[0] &&
                snap.position_ms < range[1]
              ) {
                autoSkippedRef.current[kind] = true;
                playerSeek(range[1]).catch(() => {});
              }
            }

            // Depois do encerramento, se sobra só um trecho curto (prévia do
            // próximo / créditos), vai direto pro próximo episódio. Trecho
            // longo depois do ED costuma ser história — aí não pula.
            const ending = segmentRange(skipSegments, "ending");
            if (skipSettings.nextAfterEnding && ending && !nextTriggeredRef.current && snap.duration_ms > 0) {
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
                // Lista vazia = mídia ainda não terminou de "abrir" no
                // libvlc, tenta de novo no próximo tick em vez de desistir.
                if (audioTracks.length === 0 && subtitleTracks.length === 0) return;
                autoSelectDoneRef.current = true;
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
        })
        .catch(() => {});
      // Piggyback nesse mesmo intervalo pra reafirmar o z-order do vídeo —
      // o WebView2 pode empurrar ele pra trás sozinho, ver
      // `player::window::bring_to_front` no lado Rust.
      playerBringToFront().catch(() => {});
    }, 400);
    return () => clearInterval(id);
  }, [preferredLangs, skipSettings, skipSegments]);

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

  // Solta o valor local só depois do poll já refletir o volume novo.
  function releaseVolume() {
    if (volumeReleaseTimer.current) clearTimeout(volumeReleaseTimer.current);
    volumeReleaseTimer.current = setTimeout(() => setVolumeDraft(null), 1000);
  }

  const segmentMarks: SeekSegment[] = [];
  // Trecho em que a reprodução está agora e que ainda não é pulado sozinho
  // (com pulo automático ligado ele some sozinho — botão seria redundante).
  let activeSkip: { label: string; endMs: number } | null = null;
  if (skipSegments && durationMs > 0) {
    const toPct = (ms: number) => Math.min(100, Math.max(0, (ms / durationMs) * 100));
    for (const { kind, label, skipLabel } of SEGMENT_KINDS) {
      const range = segmentRange(skipSegments, kind);
      if (!range) continue;
      if (skipSettings.mark[kind]) {
        segmentMarks.push({ startPct: toPct(range[0]), endPct: toPct(range[1]), label });
      }
      if (!skipSettings.auto[kind] && positionMs >= range[0] && positionMs < range[1]) {
        activeSkip = { label: skipLabel, endMs: range[1] };
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

  /// Atalhos estilo YouTube. `true` = tecla tratada (quem chamou evita o
  /// comportamento padrão, ex. espaço rolar a página).
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
        // Clique fora do painel de episódios fecha ele — e esse mesmo
        // clique não conta como play/pause no vídeo.
        const target = e.target as HTMLElement;
        if (openMenu === "episodes" && !target.closest("[data-episodes-panel], [data-episodes-toggle]")) {
          closingPanelAtRef.current = Date.now();
          setOpenMenu(null);
        }
      }}
      onClick={() => {
        // Horário em vez de flag: se o clique cair num controle que não
        // propaga (barra de baixo), uma flag ficaria presa e engoliria o
        // próximo clique no vídeo.
        if (Date.now() - closingPanelAtRef.current < 600) return;
        togglePause();
      }}
    >
      {/* TOPO: voltar + título/episódio, os 2 do lado esquerdo */}
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
          aria-label="Voltar"
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

      {/* CENTRO: -10s / play-pause grande / +10s — pointer-events só na
          linha dos botões, não na tela inteira (senão o "toque em qualquer
          lugar pausa" comia clique de tudo que tá embaixo/em cima dele). */}
      <div
        // pointer-events-none: essa camada cobre a tela inteira (só pra
        // centralizar os botões) e ficava por cima da barra do topo —
        // engolia o clique no "voltar" (visto no log: clique caía numa DIV).
        // Só a linha de botões recebe clique; o resto passa pro fundo
        // (clique no vídeo = play/pause).
        className="pointer-events-none absolute inset-0 flex items-center justify-center transition-opacity duration-300"
        style={{ opacity: controlsVisible ? 1 : 0 }}
      >
        <div
          className="pointer-events-auto flex items-center gap-10"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            aria-label="Voltar 10 segundos"
            onClick={() => playerSeekRelative(-SKIP_MS).catch(() => {})}
            className="relative flex size-12 items-center justify-center text-white/90 transition-colors hover:text-white"
          >
            <RotateCcw className="size-8" strokeWidth={1.6} />
            <span className="absolute text-[10px] font-bold">10</span>
          </button>
          <button
            type="button"
            aria-label={isPaused ? "Play" : "Pause"}
            onClick={togglePause}
            className="flex size-16 items-center justify-center rounded-full bg-white/15 text-white backdrop-blur-sm transition-colors hover:bg-white/25"
          >
            {isPaused ? (
              <Play className="size-7 translate-x-0.5" fill="currentColor" />
            ) : (
              <Pause className="size-7" fill="currentColor" />
            )}
          </button>
          <button
            type="button"
            aria-label="Avançar 10 segundos"
            onClick={() => playerSeekRelative(SKIP_MS).catch(() => {})}
            className="relative flex size-12 items-center justify-center text-white/90 transition-colors hover:text-white"
          >
            <RotateCw className="size-8" strokeWidth={1.6} />
            <span className="absolute text-[10px] font-bold">10</span>
          </button>
        </div>
      </div>

      {/* Botão de pular resumo/abertura/encerramento — estilo Netflix:
          aparece sozinho durante o trecho, independente do mouse. */}
      {activeSkip && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            seekTo(activeSkip.endMs);
          }}
          className="absolute right-6 bottom-24 z-10 rounded-lg border border-white/20 bg-black/60 px-4 py-2.5 text-sm font-semibold text-white backdrop-blur-sm transition-colors hover:bg-black/80"
        >
          {activeSkip.label}
        </button>
      )}

      {/* BASE: barra de progresso sólida + tempo + faixas + episódios + próximo ep + volume */}
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
            />

            <button
              type="button"
              aria-label="Episódios"
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
              aria-label="Próximo episódio"
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
                aria-label={volume > 0 ? "Mudo" : "Ativar som"}
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
              aria-label="Tela cheia"
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

/// "Temporada 3 - Episódio 1 - Título" (ver `formatPlayerTitle`) → tag
/// "Temporada 3" + resto do texto.
function EpisodeHeading({ label }: { label: string }) {
  // Tudo antes de " - Episódio" é o nome da temporada ("Temporada 3",
  // "Entertainment District Arc"...) — vira a tag.
  const match = label.match(/^(.+?)\s+-\s+(?=Episódio)/);
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

/// Barra de progresso própria (não `<input type=range>`): o range nativo só
/// limpava o "arrastando" no mouseup em cima dele — soltando fora, a agulha
/// ficava presa no último valor, até em episódio novo (bug real reportado).
/// Aqui usa pointer capture, então o soltar sempre chega. Abertura/
/// encerramento viram pedaços da própria barra (vão entre eles, estilo
/// capítulos do YouTube) com tooltip no hover.
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
  /** Arquivo local tocando — sem ele (ou stream http) não tem miniatura. */
  source: string | null;
  onScrub: (pct: number | null) => void;
  onSeek: (ms: number) => void;
}) {
  const barRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);
  const [hoverPct, setHoverPct] = useState<number | null>(null);
  const pct = durationMs > 0 ? Math.min(100, Math.max(0, (positionMs / durationMs) * 100)) : 0;

  // Miniatura do hover: quadros a cada PREVIEW_STEP_MS, gerados sob demanda
  // no Rust (~0,3–0,5s na 1ª vez) e cacheados. Só 1 pedido por vez — o
  // hover andando rápido não enfileira dezenas de decodificações, só busca
  // o ÚLTIMO ponto pedido quando o anterior termina.
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

/// Qualidade pela LARGURA (release com corte cinema tipo 1920x800 continua
/// sendo "1080p"); sem arquivo, cai pro que o nome do release diz.
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

/// Nomes únicos, filtrados pelo idioma preferido (japonês sempre passa),
/// com a tag de qualificador (Forced etc.) quando tiver.
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

// Sobrevivem a fechar/abrir o painel (a overlay nunca remonta) — reabrir
// um episódio não relê o arquivo de novo.
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
          // 30% do episódio: longe da abertura/tela preta do começo.
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
          {EPISODE_STATUS_LABEL[episode.status] ?? episode.status}
        </span>
        {quality && (
          <span className="rounded-md border border-white/20 px-1.5 py-0.5 text-[10px] font-bold text-white/80">
            {quality}
          </span>
        )}
      </div>

      {!source ? (
        <p className="text-[11px] text-white/45">Ainda não baixado — áudio e legenda aparecem quando o episódio estiver pronto.</p>
      ) : failed ? (
        <p className="text-[11px] text-white/45">Não foi possível ler o arquivo.</p>
      ) : (
        probe && (
          <>
            <ChipRow label="Áudio" chips={audio} />
            <ChipRow label="Legenda" chips={subtitles} />
          </>
        )
      )}
    </div>
  );
}

function ChipRow({ label, chips }: { label: string; chips: { label: string; original: boolean; tag: string | null }[] }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[10px] font-bold tracking-wide text-white/45 uppercase">{label}</span>
      {chips.length === 0 ? (
        <span className="text-[11px] text-white/45">Nenhuma</span>
      ) : (
        <div className="flex flex-wrap gap-1">
          {chips.map((c) => (
            <span
              key={`${c.label}|${c.tag}`}
              className="flex items-center gap-1 rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/85"
            >
              {c.label}
              {c.original && <span className="text-[9px] font-bold text-primary uppercase">Original</span>}
              {c.tag && <span className="text-[9px] font-bold text-white/55 uppercase">{c.tag}</span>}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/// Painel deslizante da direita, estilo Netflix — episódios do mesmo watch
/// (ver `NowPlaying.watch_id`) em sanfona: clicar abre imagem do episódio,
/// status, qualidade e áudio/legenda (lidos do próprio arquivo, filtrados
/// pelo idioma preferido). Play na imagem troca a mídia tocando.
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
  const [episodes, setEpisodes] = useState<Episode[]>([]);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  // Precisa do título CRU do watch (com "Season N" ainda dentro) pra montar
  // H1/H2 igual `formatPlayerTitle` espera — `snapshot.title` já vem sem a
  // season (é a SAÍDA desse mesmo formatador), não dá pra reaproveitar.
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

  // Ao abrir o painel, já expande o episódio que está tocando (igual Netflix).
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
        <span className="text-sm font-bold text-white">Episódios</span>
        <button
          type="button"
          aria-label="Fechar"
          onClick={onClose}
          className="flex size-7 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white"
        >
          <X className="size-4" />
        </button>
      </div>
      <div className="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
        {watchId == null ? (
          <p className="px-2 py-4 text-center text-xs text-white/50">Sem anime associado a esse player.</p>
        ) : sorted.length === 0 ? (
          <p className="px-2 py-4 text-center text-xs text-white/50">Nenhum episódio ainda.</p>
        ) : (
          sorted.map((ep) => {
            const expanded = expandedId === ep.id;
            const isCurrent = currentEpisode != null && numberOf(ep) === currentEpisode;
            const available = episodeSource(ep) != null;
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
                        Assistindo
                      </span>
                    )}
                  </span>
                  <span className="flex shrink-0 items-center gap-2">
                    {!expanded && (
                      <span className="text-[10px] font-bold tracking-wide text-white/50 uppercase">
                        {EPISODE_STATUS_LABEL[ep.status] ?? ep.status}
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

/// Botão de legenda/áudio na barra de baixo — popover com as 2 listas
/// (carregadas só quando abre), filtradas pelo idioma preferido (Config >
/// Reprodução) quando configurado — repack costuma vir com 10+ faixa
/// embutida, sem isso a lista fica enorme e inútil.
function TrackPickerButton({
  preferredAudio,
  preferredSubtitle,
  open,
  setOpen,
}: {
  preferredAudio: string[];
  preferredSubtitle: string[];
  /** Controlado pelo pai — só 1 menu aberto por vez (ver `openMenu`). */
  open: boolean;
  setOpen: (open: boolean) => void;
}) {
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

  // Áudio nunca mostra "Desabilitar" — silenciar o anime inteiro não faz
  // sentido em nenhum cenário (sempre tem pelo menos o original japonês).
  // Legenda continua podendo desabilitar (comum assistir dublado sem
  // legenda nenhuma) — id < 0 = entrada "Desabilitado" do próprio libvlc,
  // sempre mostra ali, não é idioma pra filtrar por preferência.
  const filteredAudio = audioTracks.filter((t) => t.id >= 0 && trackMatchesPreferred(t.name, preferredAudio));
  const filteredSubtitle = subtitleTracks.filter((t) => t.id < 0 || trackMatchesPreferred(t.name, preferredSubtitle));

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Áudio e legenda"
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
          label="Áudio"
          tracks={filteredAudio}
          onSelect={(id) => playerSetAudioTrack(id).then(() => setOpen(false))}
        />
        <div className="my-1.5 h-px bg-[#1E212A]" />
        <TrackSection
          label="Legenda"
          tracks={filteredSubtitle}
          onSelect={(id) => playerSetSubtitleTrack(id).then(() => setOpen(false))}
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
  return (
    <div className="flex flex-col gap-0.5">
      <span className="px-2 py-1 text-[10px] font-bold tracking-wide text-[#6C7180] uppercase">{label}</span>
      {tracks.length === 0 ? (
        <span className="px-2 py-1 text-xs text-[#6C7180]">Nenhuma</span>
      ) : (
        tracks.map((t) => {
          const tag = trackQualifierTag(t.name);
          const original = t.id >= 0 && isOriginalTrack(t.name);
          return (
            <button
              key={t.id}
              type="button"
              onClick={() => onSelect(t.id)}
              className={`flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors ${
                t.active ? "bg-secondary text-primary" : "hover:bg-white/5"
              }`}
            >
              <span className="flex min-w-0 items-center gap-1.5">
                <span className="truncate">{t.id < 0 ? t.name : humanizeTrackLanguage(t.name)}</span>
                {original && (
                  <span className="shrink-0 rounded-full bg-primary/20 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-primary uppercase">
                    Original
                  </span>
                )}
                {tag && (
                  <span className="shrink-0 rounded-full bg-white/10 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-white/70 uppercase">
                    {tag}
                  </span>
                )}
              </span>
              {t.active && <Check className="size-3.5 shrink-0" />}
            </button>
          );
        })
      )}
    </div>
  );
}
