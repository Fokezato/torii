import { invoke } from "@tauri-apps/api/core";

export type VlcState = "opening" | "buffering" | "playing" | "paused" | "stopped" | "ended" | "error" | "other";

export interface PlayerSnapshot {
  state: VlcState;
  position_ms: number;
  duration_ms: number;
  volume: number;
  is_playing: boolean;
  title: string;
  episode_label: string;
  watch_id: number | null;
  episode_number: number | null;
  /** Arquivo/URL tocando (pra miniatura da barra do tempo). */
  source: string;
  /** Sobe a cada abertura do player — reabrir o mesmo episódio também conta. */
  session: number;
}

export interface ProbeTrack {
  /** Código de idioma do arquivo ("jpn", "por", "pt-BR"...). */
  language: string;
  /** Nome que o release deu pra faixa ("Brazil", "Forced"...). */
  description: string;
}

export interface MediaProbe {
  duration_ms: number;
  width: number;
  height: number;
  audio: ProbeTrack[];
  subtitles: ProbeTrack[];
}

/** Duração/resolução/faixas de um arquivo, sem tocar (cacheado no Rust). */
export async function mediaProbe(path: string): Promise<MediaProbe> {
  return invoke("media_probe", { path });
}

/** Quadro do arquivo em `timeMs` como data URL JPEG (cacheado no Rust). */
export async function mediaFrame(path: string, timeMs: number): Promise<string> {
  return invoke("media_frame", { path, timeMs: Math.round(timeMs) });
}

/** Trechos de abertura/encerramento em ms — `null` num campo = esse trecho
 * não existe/não foi achado (AniSkip, capítulos do arquivo ou detecção pelo
 * áudio, ver `playerGetSkipSegments`). */
export interface SkipSegments {
  intro_start_ms: number | null;
  intro_end_ms: number | null;
  ending_start_ms: number | null;
  ending_end_ms: number | null;
  /** Resumo do episódio anterior. */
  recap_start_ms: number | null;
  recap_end_ms: number | null;
  /** Créditos por cima de cenas do episódio: mostra o botão, não pula sozinho. */
  intro_mixed: boolean;
  ending_mixed: boolean;
}

export interface TrackInfo {
  /** -1 = "Desabilitado" (legenda off / sem áudio) — já vem assim na lista. */
  id: number;
  name: string;
  active: boolean;
}

/** `source`: path local (episódio baixado) ou URL http(s) (stream).
 * `title`/`episodeLabel`/`watchId`/`episodeNumber` só alimentam a barra de
 * cima + painel de episódios + skip intro/ending do overlay. */
export async function playerOpen(
  source: string,
  title: string,
  episodeLabel: string,
  watchId: number | null,
  episodeNumber: number | null,
  /** Continuar de onde parou (ms). */
  startMs: number | null = null,
): Promise<void> {
  return invoke("player_open", { source, title, episodeLabel, watchId, episodeNumber, startMs });
}

export async function playerPlay(): Promise<void> {
  return invoke("player_play");
}

export async function playerSetPaused(paused: boolean): Promise<void> {
  return invoke("player_set_paused", { paused });
}

export async function playerStop(): Promise<void> {
  return invoke("player_stop");
}

export async function playerSeek(positionMs: number): Promise<void> {
  return invoke("player_seek", { positionMs });
}

/** `deltaMs` negativo = voltar, positivo = avançar (ex. -10000/+10000 pros
 * botões de 10s do overlay). Clampado em [0, duração] do lado Rust. */
export async function playerSeekRelative(deltaMs: number): Promise<void> {
  return invoke("player_seek_relative", { deltaMs });
}

export async function playerSetVolume(volume: number): Promise<void> {
  return invoke("player_set_volume", { volume });
}

export async function playerListAudioTracks(): Promise<TrackInfo[]> {
  return invoke("player_list_audio_tracks");
}

export async function playerSetAudioTrack(id: number): Promise<void> {
  return invoke("player_set_audio_track", { id });
}

export async function playerListSubtitleTracks(): Promise<TrackInfo[]> {
  return invoke("player_list_subtitle_tracks");
}

export async function playerSetSubtitleTrack(id: number): Promise<void> {
  return invoke("player_set_subtitle_track", { id });
}

export async function playerSnapshot(): Promise<PlayerSnapshot> {
  return invoke("player_snapshot");
}

/** x/y/width/height em pixels FÍSICOS relativos à área de conteúdo da
 * janela principal (mesma origem que `getBoundingClientRect()` do elemento
 * "slot" do vídeo no React — é filha da mesma HWND, não flutua na tela).
 * Multiplica por `window.devicePixelRatio` antes de chamar: a child HWND
 * nativa não segue o zoom/DPI do WebView2 sozinha. Ver `player/window.rs`. */
export async function playerResize(x: number, y: number, width: number, height: number): Promise<void> {
  return invoke("player_resize", { x, y, width, height });
}

export async function playerSetVisible(visible: boolean): Promise<void> {
  return invoke("player_set_visible", { visible });
}

/** Reafirma a HWND do vídeo acima do WebView2 sem reposicionar — o
 * WebView2 pode reafirmar o próprio z-order sozinho (ex. quando outra
 * janela termina de inicializar), empurrando o vídeo pra trás de novo.
 * Chamado periodicamente (ver `PlayerOverlay`), não só uma vez. */
export async function playerBringToFront(): Promise<void> {
  return invoke("player_bring_to_front");
}

/** Reposiciona a HWND do vídeo (x/y relativos à janela) E a janela de
 * overlay (overlayX/overlayY em coordenada ABSOLUTA de tela — calcule com
 * `getCurrentWindow().innerPosition()` + o mesmo rect, na mesma leitura,
 * pra não driftar durante resize/move contínuo). */
export async function playerSetVideoArea(
  x: number,
  y: number,
  width: number,
  height: number,
  overlayX: number,
  overlayY: number,
  /** Retângulo só da imagem [x, y, w, h] (luz ambiente) — sem ele o vídeo
   * ocupa a área toda. Os controles cobrem sempre a área toda. */
  video?: [number, number, number, number],
): Promise<void> {
  return invoke("player_set_video_area", { x, y, width, height, overlayX, overlayY, video: video ?? null });
}

export interface SubtitleCue {
  start_ms: number;
  end_ms: number;
  text: string;
  /** Letreiro/placa/karaokê — não desenhado no modo personalizado. */
  sign: boolean;
  italic: boolean;
  /** Fala no topo da tela. */
  top: boolean;
}

/** Falas da `ordinal`-ésima faixa de legenda do arquivo (0 = primeira),
 * extraídas com ffmpeg — base da legenda personalizada. */
export async function playerSubtitleCues(path: string, ordinal: number): Promise<SubtitleCue[]> {
  return invoke("player_subtitle_cues", { path, ordinal });
}

export interface AmbientFrame {
  /** Amostra de cores do vídeo (RGBA). */
  pixels: ImageData;
  /** Tamanho real do vídeo (proporção pro encaixe). */
  videoWidth: number;
  videoHeight: number;
}

/** Cores atuais do vídeo pra luz ambiente; `null` sem vídeo na tela. */
export async function playerAmbientFrame(): Promise<AmbientFrame | null> {
  const buffer = await invoke<ArrayBuffer>("player_ambient_frame");
  if (!buffer || buffer.byteLength < 16) return null;
  const header = new DataView(buffer, 0, 16);
  const [width, height, videoWidth, videoHeight] = [0, 4, 8, 12].map((o) => header.getUint32(o, true));
  if (width === 0 || height === 0 || buffer.byteLength < 16 + width * height * 4) return null;
  const rgba = new Uint8ClampedArray(buffer, 16, width * height * 4);
  return { pixels: new ImageData(rgba, width, height), videoWidth, videoHeight };
}

/** Salva onde parou; `watched` marca o episódio como assistido (nunca
 * desmarca) — base do "apagar depois de assistir". */
export async function playerSaveProgress(
  watchId: number,
  episodeNumber: number,
  positionMs: number,
  watched: boolean,
): Promise<void> {
  return invoke("player_save_progress", { watchId, episodeNumber, positionMs: Math.round(positionMs), watched });
}

/** Busca (com cache no backend) os trechos de abertura/encerramento do
 * episódio via AniSkip. Pode demorar (rede) na 1ª vez de um anime — chamadas
 * seguintes pro mesmo watch+episódio voltam do cache local, instantâneas. */
export async function playerGetSkipSegments(watchId: number, episodeNumber: number): Promise<SkipSegments> {
  return invoke("player_get_skip_segments", { watchId, episodeNumber });
}
