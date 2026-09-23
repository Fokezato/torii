import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";

export async function isAutostartEnabled(): Promise<boolean> {
  return isEnabled();
}

export async function setAutostart(enabled: boolean): Promise<void> {
  if (enabled) await enable();
  else await disable();
}
