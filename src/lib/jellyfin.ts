import { invoke } from "@tauri-apps/api/core";

export interface JellyfinTestResult {
  server_name: string;
  version: string;
}

export async function testJellyfinConnection(): Promise<JellyfinTestResult> {
  return invoke<JellyfinTestResult>("test_jellyfin_connection");
}
