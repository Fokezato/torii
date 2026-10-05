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
  source: string;
  session: number;
}

export interface ProbeTrack {
  language: string;
  description: string;
}

export interface MediaProbe {
  duration_ms: number;
  width: number;
  height: number;
  audio: ProbeTrack[];
  subtitles: ProbeTrack[];
}

export async function mediaProbe(path: string): Promise<MediaProbe> {
  return invoke("media_probe", { path });
}

/** Resolution from the probe, else the quality tag in the release name. */
export function videoQuality(probe: MediaProbe | undefined, releaseName: string | null): string | null {
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

export async function mediaFrame(path: string, timeMs: number): Promise<string> {
  return invoke("media_frame", { path, timeMs: Math.round(timeMs) });
}

export interface SkipSegments {
  intro_start_ms: number | null;
  intro_end_ms: number | null;
  ending_start_ms: number | null;
  ending_end_ms: number | null;
  recap_start_ms: number | null;
  recap_end_ms: number | null;
  intro_mixed: boolean;
  ending_mixed: boolean;
}

export interface TrackInfo {
  id: number;
  name: string;
  active: boolean;
}

export async function playerOpen(
  source: string,
  title: string,
  episodeLabel: string,
  watchId: number | null,
  episodeNumber: number | null,
  startMs: number | null = null,
): Promise<void> {
  return invoke("player_open", { source, title, episodeLabel, watchId, episodeNumber, startMs });
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

export async function playerBringToFront(): Promise<void> {
  return invoke("player_bring_to_front");
}

export async function playerSetVideoArea(
  x: number,
  y: number,
  width: number,
  height: number,
  overlayX: number,
  overlayY: number,
  video?: [number, number, number, number],
): Promise<void> {
  return invoke("player_set_video_area", { x, y, width, height, overlayX, overlayY, video: video ?? null });
}

export interface SubtitleCue {
  start_ms: number;
  end_ms: number;
  text: string;
  sign: boolean;
  italic: boolean;
  top: boolean;
}

export async function playerSubtitleCues(path: string, ordinal: number): Promise<SubtitleCue[]> {
  return invoke("player_subtitle_cues", { path, ordinal });
}

export interface AmbientFrame {
  pixels: ImageData;
  videoWidth: number;
  videoHeight: number;
}

export async function playerAmbientFrame(): Promise<AmbientFrame | null> {
  const buffer = await invoke<ArrayBuffer>("player_ambient_frame");
  if (!buffer || buffer.byteLength < 16) return null;
  const header = new DataView(buffer, 0, 16);
  const [width, height, videoWidth, videoHeight] = [0, 4, 8, 12].map((o) => header.getUint32(o, true));
  if (width === 0 || height === 0 || buffer.byteLength < 16 + width * height * 4) return null;
  const rgba = new Uint8ClampedArray(buffer, 16, width * height * 4);
  return { pixels: new ImageData(rgba, width, height), videoWidth, videoHeight };
}

export async function playerSaveProgress(
  watchId: number,
  episodeNumber: number,
  positionMs: number,
  watched: boolean,
): Promise<void> {
  return invoke("player_save_progress", { watchId, episodeNumber, positionMs: Math.round(positionMs), watched });
}

export async function playerGetSkipSegments(watchId: number, episodeNumber: number): Promise<SkipSegments> {
  return invoke("player_get_skip_segments", { watchId, episodeNumber });
}
