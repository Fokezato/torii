import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { emit, listen } from "@tauri-apps/api/event";
import { getSettings } from "@/lib/tauri";
import ptBR from "./pt-BR";
import en from "./en";

export type AppLanguage = "pt-BR" | "en";

export function resolveLanguage(setting?: string | null): AppLanguage {
  if (setting === "pt-BR" || setting === "en") return setting;
  return navigator.language.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

const STORAGE_KEY = "torii.language";

function storedSetting(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

i18n.use(initReactI18next).init({
  resources: { "pt-BR": { translation: ptBR }, en: { translation: en } },
  lng: resolveLanguage(storedSetting()),
  fallbackLng: "pt-BR",
  interpolation: { escapeValue: false },
});

function applyLanguage(setting?: string | null) {
  try {
    if (setting) localStorage.setItem(STORAGE_KEY, setting);
  } catch {
  }
  const lng = resolveLanguage(setting);
  document.documentElement.lang = lng;
  if (i18n.language !== lng) i18n.changeLanguage(lng);
}

export function syncLanguage(): Promise<void> {
  listen<string>("app:language-changed", (event) => applyLanguage(event.payload)).catch(() => {});
  const loaded = getSettings()
    .then((s) => applyLanguage(s.app_language))
    .catch(() => {});
  const timeout = new Promise<void>((resolve) => setTimeout(resolve, 1500));
  return Promise.race([loaded, timeout]);
}

export function broadcastLanguage(setting: string) {
  applyLanguage(setting);
  emit("app:language-changed", setting).catch(() => {});
}

export function currentLocale(): string {
  return i18n.language === "en" ? "en-US" : "pt-BR";
}

export const t = i18n.t.bind(i18n);

export function tDynamic(key: string, fallback: string): string {
  return i18n.exists(key) ? (i18n.t as unknown as (k: string) => string)(key) : fallback;
}

export default i18n;
