import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { emit, listen } from "@tauri-apps/api/event";
import { getSettings } from "@/lib/tauri";
import ptBR from "./pt-BR";
import en from "./en";

export type AppLanguage = "pt-BR" | "en";

/// Configuração `app_language`: "pt-BR", "en" ou "auto"/vazio (segue o
/// idioma do Windows — português se o sistema estiver em português, senão
/// inglês).
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
    // Sem storage: só não lembra pro próximo boot.
  }
  const lng = resolveLanguage(setting);
  document.documentElement.lang = lng;
  if (i18n.language !== lng) i18n.changeLanguage(lng);
}

/// Cada janela (principal, controles do player, notificação) tem o próprio
/// i18n: lê a configuração no boot e escuta a troca feita em Config. A
/// promessa resolve quando o idioma salvo já foi aplicado (ou desistiu),
/// pra tela não nascer no idioma errado.
export function syncLanguage(): Promise<void> {
  listen<string>("app:language-changed", (event) => applyLanguage(event.payload)).catch(() => {});
  const loaded = getSettings()
    .then((s) => applyLanguage(s.app_language))
    .catch(() => {});
  const timeout = new Promise<void>((resolve) => setTimeout(resolve, 1500));
  return Promise.race([loaded, timeout]);
}

/// Chamado por Config ao trocar o idioma — atualiza as outras janelas.
export function broadcastLanguage(setting: string) {
  applyLanguage(setting);
  emit("app:language-changed", setting).catch(() => {});
}

/// Locale pra datas/números (Intl/toLocaleDateString).
export function currentLocale(): string {
  return i18n.language === "en" ? "en-US" : "pt-BR";
}

export const t = i18n.t.bind(i18n);

/// Chave montada em tempo de execução (ex. "languages.Japanese", a partir
/// de um valor vindo da AniList/banco) — cai em `fallback` se não existir.
export function tDynamic(key: string, fallback: string): string {
  return i18n.exists(key) ? (i18n.t as unknown as (k: string) => string)(key) : fallback;
}

export default i18n;
