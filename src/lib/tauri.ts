import { invoke } from "@tauri-apps/api/core";

export async function getSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("get_settings");
}

export async function updateSettings(values: Record<string, string>): Promise<void> {
  return invoke<void>("update_settings", { values });
}
