import { invoke } from "@tauri-apps/api/core";

export interface LogEntry {
  id: number;
  timestamp: string;
  level: "info" | "error";
  message: string;
}

export async function getActivityLog(): Promise<LogEntry[]> {
  return invoke<LogEntry[]>("get_activity_log");
}
