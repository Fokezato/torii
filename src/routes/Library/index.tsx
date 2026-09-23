import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { groupBySeries, listWatches, type WatchGroup } from "@/lib/watches";
import { Skeleton } from "@/components/ui/skeleton";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { LibraryCard } from "@/components/library/LibraryCard";
import { useSearchStore } from "@/stores/search";
import { findContinueEpisode, playEpisode } from "@/lib/continueWatching";

const CARD_SIZES = [
  { id: "sm", labelKey: "library.sizeSmall", width: 290 },
  { id: "md", labelKey: "library.sizeMedium", width: 360 },
  { id: "lg", labelKey: "library.sizeLarge", width: 430 },
] as const;
type CardSizeId = (typeof CARD_SIZES)[number]["id"];
const CARD_SIZE_STORAGE_KEY = "torii:library-card-size";

function useCardSize() {
  const [size, setSize] = useState<CardSizeId>("md");
  useEffect(() => {
    try {
      const saved = localStorage.getItem(CARD_SIZE_STORAGE_KEY);
      if (saved === "sm" || saved === "md" || saved === "lg") setSize(saved);
    } catch {
      // localStorage indisponível (ex. contexto privado) — mantém o padrão
    }
  }, []);
  const update = (next: CardSizeId) => {
    setSize(next);
    try {
      localStorage.setItem(CARD_SIZE_STORAGE_KEY, next);
    } catch {
      // best-effort, sem problema se não persistir
    }
  };
  return [size, update] as const;
}

const FILTERS = [
  { id: "all", labelKey: "library.filterAll" },
  { id: "watching", labelKey: "listStatus.watching" },
  { id: "downloaded", labelKey: "library.filterDownloaded" },
  { id: "completed", labelKey: "listStatus.completed" },
  { id: "planning", labelKey: "listStatus.planning" },
] as const;
type FilterId = (typeof FILTERS)[number]["id"];

const SORTS = [
  { id: "updated", labelKey: "library.sortUpdated" },
  { id: "name", labelKey: "library.sortName" },
  { id: "rating", labelKey: "library.sortRating" },
] as const;
type SortId = (typeof SORTS)[number]["id"];

export default function Library() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isLoading } = useQuery({ queryKey: ["watches"], queryFn: listWatches });
  const [filter, setFilter] = useState<FilterId>("all");
  const [sort, setSort] = useState<SortId>("updated");
  const search = useSearchStore((s) => s.term);
  const [cardSize, setCardSize] = useCardSize();
  const cardWidth = CARD_SIZES.find((s) => s.id === cardSize)!.width;

  const groups = useMemo(() => groupBySeries(data ?? []), [data]);

  // Play no card: continua de onde parou (1º episódio baixado e não
  // assistido). Nada baixado ainda → abre a página do anime.
  const [playingKey, setPlayingKey] = useState<string | null>(null);
  async function playGroup(g: WatchGroup) {
    if (playingKey) return;
    setPlayingKey(g.key);
    try {
      const target = await findContinueEpisode(g.seasons);
      if (target) await playEpisode(target.watch, target.episode, navigate);
      else navigate(`/library/${g.representative.id}`);
    } catch {
      navigate(`/library/${g.representative.id}`);
    } finally {
      setPlayingKey(null);
    }
  }

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: groups.length };
    for (const g of groups) c[g.representative.list_status] = (c[g.representative.list_status] ?? 0) + 1;
    return c;
  }, [groups]);

  const filtered = useMemo(() => {
    let list = groups;
    if (filter !== "all") list = list.filter((g) => g.representative.list_status === filter);
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      list = list.filter((g) => g.title.toLowerCase().includes(q));
    }
    const sorted = [...list];
    if (sort === "name") sorted.sort((a, b) => a.title.localeCompare(b.title));
    else if (sort === "rating") sorted.sort((a, b) => (b.representative.rating ?? -1) - (a.representative.rating ?? -1));
    else
      sorted.sort(
        (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
      );
    return sorted;
  }, [groups, filter, search, sort]);

  return (
    <div className="flex flex-col gap-[22px]">
      <div className="flex items-baseline justify-between">
        <div className="flex items-baseline gap-3">
          <h1 className="text-[26px] font-bold">{t("library.title")}</h1>
          <span className="text-[13px] text-[#6C7180]">{t("library.savedCount", { count: groups.length })}</span>
        </div>
        <div className="flex items-center gap-2.5">
          <div
            role="group"
            aria-label={t("library.cardSize")}
            className="flex items-center gap-0.5 rounded-lg border border-[#21242C] bg-[#15171D] p-0.5"
          >
            {CARD_SIZES.map((s, i) => (
              <button
                key={s.id}
                type="button"
                title={t(s.labelKey)}
                aria-label={t(s.labelKey)}
                aria-pressed={cardSize === s.id}
                onClick={() => setCardSize(s.id)}
                className={`flex size-7 items-center justify-center rounded-[6px] transition-colors ${
                  cardSize === s.id ? "bg-primary text-primary-foreground" : "text-[#6C7180] hover:text-foreground"
                }`}
              >
                <span
                  className="rounded-[2px] bg-current"
                  style={{ width: 6 + i * 3, height: 6 + i * 3 }}
                  aria-hidden="true"
                />
              </button>
            ))}
          </div>
          <Select value={sort} onValueChange={(v) => setSort(v as SortId)}>
            <SelectTrigger
              size="sm"
              style={{ backgroundColor: "#15171D", borderColor: "#21242C" }}
              className="rounded-lg px-3 text-xs"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SORTS.map((s) => (
                <SelectItem key={s.id} value={s.id}>
                  {t(s.labelKey)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <nav aria-label={t("library.filter")} className="flex items-center gap-2">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            type="button"
            aria-pressed={filter === f.id}
            onClick={() => setFilter(f.id)}
            className={`rounded-full px-4 py-2 text-[13px] font-semibold transition-colors ${
              filter === f.id
                ? "bg-primary text-primary-foreground"
                : "border border-[#23262F] text-[#B5B9C4] hover:text-foreground"
            }`}
          >
            {t(f.labelKey)} · {counts[f.id] ?? 0}
          </button>
        ))}
      </nav>

      {isLoading && (
        <div className="flex flex-wrap gap-x-[18px] gap-y-5">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="space-y-2.5" style={{ width: cardWidth }}>
              <Skeleton className="rounded-xl" style={{ width: cardWidth, height: Math.round(cardWidth * (124 / 220)) }} />
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="h-3 w-1/2" />
            </div>
          ))}
        </div>
      )}

      {!isLoading && (data?.length ?? 0) === 0 && (
        <p className="text-sm text-muted-foreground">
          {t("library.empty")}
        </p>
      )}

      {!isLoading && (data?.length ?? 0) > 0 && filtered.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("library.noMatch")}</p>
      )}

      {!isLoading && filtered.length > 0 && (
        <div className="flex flex-wrap gap-x-[18px] gap-y-5">
          {filtered.map((g) => (
            <LibraryCard
              key={g.key}
              watch={g.representative}
              title={g.title}
              seasonCount={g.seasons.length}
              width={cardWidth}
              playLoading={playingKey === g.key}
              onPlay={() => playGroup(g)}
              onOpenDetails={() => navigate(`/library/${g.representative.id}`)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
