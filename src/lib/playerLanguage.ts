import type { TrackInfo } from "@/lib/player";
import { languageLabel, languageOptions } from "@/lib/constants";
import { t } from "@/i18n";

const REGION_TO_CANONICAL: Record<string, string> = {
  brazil: "Brazil",
  "latin america": "Latin America",
  spain: "Spain",
  simplified: "Simplified",
  traditional: "Traditional",
  "hong kong": "Traditional",
  taiwan: "Traditional",
  mainland: "Simplified",
};

export function humanizeTrackLanguage(rawName: string): string {
  const bracketMatch = rawName.match(/\[([^\]]+)\]\s*$/);
  const language = (bracketMatch ? bracketMatch[1] : rawName).trim();
  const prefix = bracketMatch ? rawName.slice(0, bracketMatch.index).replace(/-\s*$/, "").trim() : "";

  let region: string | null = null;
  if (prefix && !/^track\s*\d+$/i.test(prefix)) {
    for (const part of prefix.split("|").map((p) => p.trim().toLowerCase())) {
      if (REGION_TO_CANONICAL[part]) {
        region = REGION_TO_CANONICAL[part];
        break;
      }
    }
  }

  const canonical = region ? `${language} (${region})` : language;
  const known = languageOptions().find((l) => l.value.toLowerCase() === canonical.toLowerCase());
  return known ? known.label : languageLabel(canonical);
}

function baseLanguageWord(canonical: string): string {
  return canonical.replace(/\s*\(.*\)\s*$/, "").trim().toLowerCase();
}

export function trackMatchesPreferred(trackName: string, preferred: string[]): boolean {
  if (preferred.length === 0) return true;
  const lower = trackName.toLowerCase();
  if (lower.includes("japanese")) return true;
  return preferred.some((lang) => lower.includes(baseLanguageWord(lang)));
}

export function findPreferredTrack(tracks: TrackInfo[], preferred: string[]): TrackInfo | null {
  for (const lang of preferred) {
    const base = baseLanguageWord(lang);
    const found = tracks.find((t) => t.id >= 0 && t.name.toLowerCase().includes(base));
    if (found) return found;
  }
  return null;
}

const ISO_LANGUAGE: Record<string, string> = {
  jpn: "Japanese", ja: "Japanese",
  eng: "English", en: "English",
  por: "Portuguese", pt: "Portuguese",
  spa: "Spanish", es: "Spanish",
  fre: "French", fra: "French", fr: "French",
  ger: "German", deu: "German", de: "German",
  ita: "Italian", it: "Italian",
  rus: "Russian", ru: "Russian",
  ara: "Arabic", ar: "Arabic",
  chi: "Chinese", zho: "Chinese", zh: "Chinese",
  pol: "Polish", pl: "Polish",
  ind: "Indonesian", id: "Indonesian",
  may: "Malay", msa: "Malay", ms: "Malay",
  tha: "Thai", th: "Thai",
  vie: "Vietnamese", vi: "Vietnamese",
  kor: "Korean", ko: "Korean",
  hin: "Hindi", hi: "Hindi",
  tur: "Turkish", tr: "Turkish",
};

const ISO_REGION: Record<string, string> = {
  br: "Brazil",
  "419": "Latin America",
  mx: "Latin America",
  es: "Spain",
  cn: "Simplified",
  hans: "Simplified",
  tw: "Traditional",
  hk: "Traditional",
  hant: "Traditional",
};

export function probeTrackName(language: string, description: string): string {
  const [code = "", region] = language.toLowerCase().split(/[-_]/);
  const lang = ISO_LANGUAGE[code] ?? (language || "Unknown");
  const regionWord = region ? ISO_REGION[region] : undefined;
  const prefix = [regionWord, description.trim()].filter(Boolean).join(" | ");
  return prefix ? `${prefix} - [${lang}]` : `[${lang}]`;
}

export function isOriginalTrack(trackName: string): boolean {
  return trackName.toLowerCase().includes("japanese");
}

export function trackQualifierTag(trackName: string): string | null {
  const lower = trackName.toLowerCase();
  if (lower.includes("forced")) return "Forced";
  if (lower.includes("sdh")) return "SDH";
  if (lower.includes("descriptive") || lower.includes("descrição") || lower.includes("descritiv"))
    return t("player.tagDescriptive");
  if (lower.includes("commentary") || lower.includes("comentário")) return t("player.tagCommentary");
  if (lower.includes("karaoke") || lower.includes("signs") || lower.includes("songs")) return "Signs/Songs";
  return null;
}
