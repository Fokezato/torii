import { invoke } from "@tauri-apps/api/core";

export interface JellyfinTestResult {
  server_name: string;
  version: string;
}

export interface JellyfinUser {
  id: string;
  name: string;
}

export async function jellyfinUsers(): Promise<JellyfinUser[]> {
  return invoke<JellyfinUser[]>("jellyfin_users");
}

export async function testJellyfinConnection(): Promise<JellyfinTestResult> {
  return invoke<JellyfinTestResult>("test_jellyfin_connection");
}
