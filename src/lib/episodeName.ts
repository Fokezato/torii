import { seasonTabLabel } from "@/lib/anilist";
import { t } from "@/i18n";

const SXXEYY = /S(\d{1,2})E(\d{1,3})/i;
const VIDEO_EXTENSIONS = /\.(mkv|mp4|avi|webm|mov|m4v|wmv|flv|ts)$/i;

/** Nome de EXIBIÇÃO nunca mostra extensão de arquivo — isso é detalhe de
 * caminho em disco, não informação pro usuário. Regra vale pra qualquer
 * lugar que possa acabar recebendo um nome de arquivo cru (ex. página de
 * teste do player, que abre arquivo escolhido direto pelo usuário). */
export function stripFileExtension(filename: string): string {
  return filename.replace(VIDEO_EXTENSIONS, "");
}

/** "[ToonsHub] Show S03E02 1080p CR WEB-DL MULTi..." -> "Episódio 2".
 * `episodeNumber` é o fallback pra placeholder ainda sem release associado
 * (nome cru é null até o motor achar um torrent — ver `episodes.episode_number`). */
export function parseEpisodeLabel(rawName: string | null, episodeNumber?: number | null): string {
  if (!rawName) return episodeNumber != null ? t("labels.episode", { number: episodeNumber }) : t("labels.episodeWord");
  const clean = stripFileExtension(rawName);
  const match = clean.match(SXXEYY);
  if (!match) return clean;
  return t("labels.episode", { number: parseInt(match[2], 10) });
}

/** Deriva "nome do anime" + "Episódio N - Título" a partir de um nome de
 * arquivo cru, pra quando não tem metadado de verdade associado (ex.
 * página de teste do player sem watch escolhido). Ex.: "Mushoku Tensei
 * S03E01 Burn Bright Mad Dog.mkv" -> title "Mushoku Tensei", episodeLabel
 * "Episódio 1 - Burn Bright Mad Dog". */
export function parseFilenameTitleAndEpisode(rawFilename: string): { title: string; episodeLabel: string } {
  const clean = stripFileExtension(rawFilename);
  const match = clean.match(SXXEYY);
  if (!match || match.index == null) {
    return { title: clean.trim(), episodeLabel: "" };
  }
  const title = clean
    .slice(0, match.index)
    // corta tag(s) de grupo de release no INÍCIO do nome ("[Feibanyama] ",
    // "(SubsPlease) "...) — sem isso sobrava colada no título exibido.
    .replace(/^(?:\s*[[(][^\])]*[\])])+\s*/, "")
    .replace(/[-_[(]+$/, "")
    .trim();
  const episodeNumber = parseInt(match[2], 10);
  const rest = clean
    .slice(match.index + match[0].length)
    // corta tag de qualidade/grupo/release que costuma vir depois do nome
    // do episódio ("1080p", "[Grupo]", "(CR WEB-DL)"...).
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
  return match ? parseInt(match[2], 10) : null;
}

export function parseSeasonFromEpisodeName(rawName: string | null): number | null {
  if (!rawName) return null;
  const match = rawName.match(SXXEYY);
  return match ? parseInt(match[1], 10) : null;
}

/** "Show Name Season 3" / "Show Name 3rd Season" -> 3 */
export function parseSeasonFromTitle(title: string | null): number | null {
  if (!title) return null;
  const match = title.match(/season\s*(\d+)|(\d+)(?:st|nd|rd|th)\s*season/i);
  if (!match) return null;
  const n = match[1] ?? match[2];
  return n ? parseInt(n, 10) : null;
}

/** "Re:ZERO -...- Season 4" -> "Re:ZERO -...-" — mesma regra do backend
 * (src-tauri/src/sources/nyaa.rs::split_season), usada aqui só pra exibir/
 * agrupar, não pra montar pasta em disco. */
export function stripSeasonSuffix(title: string): string {
  return title.replace(/\s*(?:season\s*\d+|\d+(?:st|nd|rd|th)\s*season)\s*$/i, "").trim();
}

/** Chave de agrupamento por anime base, ignorando qual season é. */
export function getSeriesGroupKey(title: string): string {
  return stripSeasonSuffix(title).toLowerCase();
}

/** Cabeçalho do player: H1 = nome do anime sem "Season N" (isso vai pro
 * H2), H2 = "Temporada N - Episódio X - Título". `animeTitle` é o título
 * do watch (ex. "Grand Blue Dreaming Season 3"), `episodeLabel` já vem
 * pronto de `parseFilenameTitleAndEpisode`/`parseEpisodeLabel` (ex.
 * "Episódio 3" ou "Episódio 3 - Título"). */
export function formatPlayerTitle(
  animeTitle: string,
  episodeLabel: string,
  seriesTitle?: string | null,
): { title: string; episodeLabel: string } {
  // Com o anime resolvido (franquia na AniList): H1 = nome do anime, prefixo
  // do H2 = nome da temporada/arco (ex. "Entertainment District Arc").
  if (seriesTitle) {
    return { title: seriesTitle, episodeLabel: `${seasonTabLabel(animeTitle, seriesTitle)} - ${episodeLabel}` };
  }
  const season = parseSeasonFromTitle(animeTitle);
  return {
    title: stripSeasonSuffix(animeTitle),
    episodeLabel: season != null ? `${t("labels.season", { number: season })} - ${episodeLabel}` : episodeLabel,
  };
}
