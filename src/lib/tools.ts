import { invoke } from "@tauri-apps/api/core";

export interface FfmpegStatus {
  installed: boolean;
  downloading: boolean;
  /** 0–100 enquanto baixa. */
  progress: number;
  error: string | null;
}

export async function ffmpegStatus(): Promise<FfmpegStatus> {
  return invoke("ffmpeg_status");
}

/** Baixa o ffmpeg em segundo plano (volta na hora); acompanhe via `ffmpegStatus`. */
export async function ffmpegInstall(): Promise<void> {
  return invoke("ffmpeg_install");
}

/** Tradução automática com cache no banco; devolve o original se falhar. */
export async function translateText(text: string, target: string): Promise<string> {
  return invoke("translate_text", { text, target });
}
