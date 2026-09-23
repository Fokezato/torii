import { invoke } from "@tauri-apps/api/core";

// Placeholder round-trip check for the scaffolding phase.
// Will be replaced by real typed commands (settings, watches, downloads, season...) in later phases.
export async function pingBackend(): Promise<string> {
  return invoke<string>("greet", { name: "Torii" });
}

export async function getSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("get_settings");
}

export async function updateSettings(values: Record<string, string>): Promise<void> {
  return invoke<void>("update_settings", { values });
}
