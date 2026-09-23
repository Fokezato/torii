export const LANGUAGES = [
  { value: "any", label: "(qualquer)" },
  { value: "Japanese", label: "Japonês" },
  { value: "English", label: "Inglês" },
  { value: "Portuguese (Brazil)", label: "Português (Brasil)" },
  { value: "Spanish (Latin America)", label: "Espanhol (LatAm)" },
  { value: "Spanish (Spain)", label: "Espanhol (Espanha)" },
  { value: "French", label: "Francês" },
  { value: "German", label: "Alemão" },
  { value: "Italian", label: "Italiano" },
  { value: "Russian", label: "Russo" },
  { value: "Arabic", label: "Árabe" },
  { value: "Chinese (Simplified)", label: "Chinês (Simplificado)" },
  { value: "Chinese (Traditional)", label: "Chinês (Tradicional)" },
  { value: "Polish", label: "Polonês" },
  { value: "Indonesian", label: "Indonésio" },
  { value: "Malay", label: "Malaio" },
  { value: "Thai", label: "Tailandês" },
  { value: "Vietnamese", label: "Vietnamita" },
] as const;

export const QUALITIES = [
  { value: "any", label: "Qualquer" },
  { value: "480p", label: "480p" },
  { value: "720p", label: "720p" },
  { value: "1080p", label: "1080p" },
  { value: "2160p", label: "2160p" },
] as const;

export const STATUS_LABEL: Record<string, string> = {
  FINISHED: "Finalizado",
  RELEASING: "Em exibição",
  NOT_YET_RELEASED: "Anunciado",
  CANCELLED: "Cancelado",
  HIATUS: "Em hiato",
};

export const SEASON_LABEL: Record<string, string> = {
  WINTER: "Inverno",
  SPRING: "Primavera",
  SUMMER: "Verão",
  FALL: "Outono",
};

export const LIST_STATUS_LABEL: Record<string, string> = {
  watching: "Assistindo",
  downloaded: "Baixado",
  completed: "Concluído",
  planning: "Quero assistir",
};

export const LIST_STATUS_OPTIONS = [
  { value: "watching", label: "Assistindo" },
  { value: "downloaded", label: "Baixado" },
  { value: "completed", label: "Concluído" },
  { value: "planning", label: "Quero assistir" },
] as const;
