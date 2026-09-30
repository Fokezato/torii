import { emit, listen } from "@tauri-apps/api/event";

export interface AmbientSettings {
  enabled: boolean;
  intensity: number;
  size: number;
  smoothness: number;
}

export const DEFAULT_AMBIENT: AmbientSettings = { enabled: true, intensity: 85, size: 135, smoothness: 35 };

export function ambientFromSettings(s: Record<string, string>): AmbientSettings {
  const num = (key: string, fallback: number, min: number, max: number) => {
    const v = Number(s[key]);
    return s[key] != null && s[key] !== "" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback;
  };
  return {
    enabled: s.player_ambient_light !== "0",
    intensity: num("player_ambient_intensity", DEFAULT_AMBIENT.intensity, 0, 100),
    size: num("player_ambient_size", DEFAULT_AMBIENT.size, 110, 180),
    smoothness: num("player_ambient_smoothness", DEFAULT_AMBIENT.smoothness, 0, 100),
  };
}

export function ambientToSettings(a: AmbientSettings): Record<string, string> {
  return {
    player_ambient_light: a.enabled ? "1" : "0",
    player_ambient_intensity: String(a.intensity),
    player_ambient_size: String(a.size),
    player_ambient_smoothness: String(a.smoothness),
  };
}

const EVENT = "player:ambient-changed";

export function broadcastAmbient(a: AmbientSettings) {
  emit(EVENT, a).catch(() => {});
}

export function onAmbientChanged(cb: (a: AmbientSettings) => void) {
  return listen<AmbientSettings>(EVENT, (e) => cb(e.payload));
}
