import { invoke } from "@tauri-apps/api/core";

export interface NewsItem {
  title: string;
  link: string;
  summary: string | null;
  category: string | null;
  published_at: string | null;
}

export async function getAnimeNews(): Promise<NewsItem[]> {
  return invoke<NewsItem[]>("get_anime_news");
}
