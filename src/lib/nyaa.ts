import { invoke } from "@tauri-apps/api/core";

export interface AvailableLanguages {
  audio: string[];
  subtitles: string[];
}

export async function getAvailableLanguages(query: string): Promise<AvailableLanguages> {
  return invoke<AvailableLanguages>("nyaa_available_languages", { query });
}
