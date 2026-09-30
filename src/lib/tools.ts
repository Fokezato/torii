import { invoke } from "@tauri-apps/api/core";

export interface FfmpegStatus {
  installed: boolean;
  downloading: boolean;
  progress: number;
  error: string | null;
}

export async function ffmpegStatus(): Promise<FfmpegStatus> {
  return invoke("ffmpeg_status");
}

export async function ffmpegInstall(): Promise<void> {
  return invoke("ffmpeg_install");
}

export async function translateText(text: string, target: string): Promise<string> {
  return invoke("translate_text", { text, target });
}
