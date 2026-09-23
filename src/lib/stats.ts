import { invoke } from "@tauri-apps/api/core";

export interface StorageStats {
  used_bytes: number;
}

export async function getStorageStats(): Promise<StorageStats> {
  return invoke<StorageStats>("get_storage_stats");
}
