import { seasonTabLabel } from "@/lib/anilist";
import { t } from "@/i18n";

const SXXEYY = /S(\d{1,2})E(\d{1,3})/i;
// "Show - 01 [1080p]", "Show - 12v2 (WEB)"
const DASH_EPISODE = /\s-\s(\d{1,4})(?:v\d)?(?=\s*[[(]|\s*$)/;
const VIDEO_EXTENSIONS = /\.(mkv|mp4|avi|webm|mov|m4v|wmv|flv|ts)$/i;

export function stripFileExtension(filename: string): string {
  return filename.replace(VIDEO_EXTENSIONS, "");
}

export function parseEpisodeLabel(rawName: string | null, episodeNumber?: number | null): string {
  if (!rawName) return episodeNumber != null ? t("labels.episode", { number: episodeNumber }) : t("labels.episodeWord");
  const number = episodeNumber ?? parseEpisodeNumber(rawName);
  return number != null ? t("labels.episode", { number }) : stripFileExtension(rawName);
}

export function parseFilenameTitleAndEpisode(rawFilename: string): { title: string; episodeLabel: string } {
  const clean = stripFileExtension(rawFilename);
  const match = clean.match(SXXEYY);
  if (!match || match.index == null) {
    return { title: clean.trim(), episodeLabel: "" };
  }
  const title = clean
    .slice(0, match.index)
    .replace(/^(?:\s*[[(][^\])]*[\])])+\s*/, "")
    .replace(/[-_[(]+$/, "")
    .trim();
  const episodeNumber = parseInt(match[2], 10);
  const rest = clean
    .slice(match.index + match[0].length)
    .replace(/[[({].*$/, "")
    .replace(/^[\s._-]+|[\s._-]+$/g, "")
    .trim();
  return {
    title: title || clean.trim(),
    episodeLabel: rest
      ? t("labels.episodeWithTitle", { number: episodeNumber, title: rest })
      : t("labels.episode", { number: episodeNumber }),
  };
}

export function parseEpisodeNumber(rawName: string | null): number | null {
  if (!rawName) return null;
  const match = rawName.match(SXXEYY);
  if (match) return parseInt(match[2], 10);
  const dashed = rawName.match(DASH_EPISODE);
  return dashed ? parseInt(dashed[1], 10) : null;
}

export function parseSeasonFromEpisodeName(rawName: string | null): number | null {
  if (!rawName) return null;
  const match = rawName.match(SXXEYY);
  return match ? parseInt(match[1], 10) : null;
}

export function parseSeasonFromTitle(title: string | null): number | null {
  if (!title) return null;
  const match = title.match(/season\s*(\d+)|(\d+)(?:st|nd|rd|th)\s*season/i);
  if (!match) return null;
  const n = match[1] ?? match[2];
  return n ? parseInt(n, 10) : null;
}

export function stripSeasonSuffix(title: string): string {
  return title.replace(/\s*(?:season\s*\d+|\d+(?:st|nd|rd|th)\s*season)\s*$/i, "").trim();
}

export function getSeriesGroupKey(title: string): string {
  return stripSeasonSuffix(title).toLowerCase();
}

export function formatPlayerTitle(
  animeTitle: string,
  episodeLabel: string,
  seriesTitle?: string | null,
): { title: string; episodeLabel: string } {
  if (seriesTitle) {
    return { title: seriesTitle, episodeLabel: `${seasonTabLabel(animeTitle, seriesTitle)} - ${episodeLabel}` };
  }
  const season = parseSeasonFromTitle(animeTitle);
  return {
    title: stripSeasonSuffix(animeTitle),
    episodeLabel: season != null ? `${t("labels.season", { number: season })} - ${episodeLabel}` : episodeLabel,
  };
}
