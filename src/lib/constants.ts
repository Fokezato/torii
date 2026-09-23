import { t, tDynamic } from "@/i18n";

// Valores canônicos (os mesmos gravados no banco e usados na busca do
// Nyaa); o rótulo mostrado vem da tradução (`languages.*`).
const LANGUAGE_VALUES = [
  "any",
  "Japanese",
  "English",
  "Portuguese (Brazil)",
  "Spanish (Latin America)",
  "Spanish (Spain)",
  "French",
  "German",
  "Italian",
  "Russian",
  "Arabic",
  "Chinese (Simplified)",
  "Chinese (Traditional)",
  "Polish",
  "Indonesian",
  "Malay",
  "Thai",
  "Vietnamese",
] as const;

export function languageLabel(value: string): string {
  return tDynamic(`languages.${value}`, value);
}

export function languageOptions(): { value: string; label: string }[] {
  return LANGUAGE_VALUES.map((value) => ({ value, label: languageLabel(value) }));
}

export function qualityOptions(): { value: string; label: string }[] {
  return [
    { value: "any", label: t("quality.any") },
    { value: "480p", label: "480p" },
    { value: "720p", label: "720p" },
    { value: "1080p", label: "1080p" },
    { value: "2160p", label: "2160p" },
  ];
}

export function qualityLabel(value: string): string {
  return qualityOptions().find((q) => q.value === value)?.label ?? value;
}

/// Status de exibição na AniList (FINISHED, RELEASING...).
export function statusLabel(status: string): string {
  return tDynamic(`animeStatus.${status}`, status);
}

/// Estação do ano da AniList (WINTER, SPRING...).
export function seasonLabel(season: string): string {
  return tDynamic(`seasons.${season}`, season);
}

/// Status do episódio no banco (pending, downloading, available...).
export function episodeStatusLabel(status: string): string {
  return tDynamic(`episodeStatus.${status}`, status);
}

const LIST_STATUS_VALUES = ["watching", "downloaded", "completed", "planning"] as const;

/// Status do anime na lista do usuário (Assistindo, Baixado...).
export function listStatusLabel(status: string): string {
  return tDynamic(`listStatus.${status}`, status);
}

export function listStatusOptions(): { value: string; label: string }[] {
  return LIST_STATUS_VALUES.map((value) => ({ value, label: listStatusLabel(value) }));
}

/// Gênero da AniList traduzido (gênero desconhecido aparece como veio).
export function genreLabel(genre: string): string {
  return tDynamic(`genres.${genre}`, genre);
}
