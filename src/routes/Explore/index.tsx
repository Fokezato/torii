import { useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Loader2, Plus, Star } from "lucide-react";
import { anilistCatalog, type AnimeSummary, type CatalogFilter } from "@/lib/anilist";
import { AnimeDetailModal } from "@/components/anime/AnimeDetailModal";
import { WatchFormModal } from "@/components/anime/WatchFormModal";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { currentLocale, tDynamic } from "@/i18n";
import { genreLabel, seasonLabel, statusLabel } from "@/lib/constants";
import { useSearchStore } from "@/stores/search";
import { cn } from "@/lib/utils";

const GENRES = [
  "Action",
  "Adventure",
  "Comedy",
  "Drama",
  "Ecchi",
  "Fantasy",
  "Horror",
  "Mahou Shoujo",
  "Mecha",
  "Music",
  "Mystery",
  "Psychological",
  "Romance",
  "Sci-Fi",
  "Slice of Life",
  "Sports",
  "Supernatural",
  "Thriller",
];
const STATUSES = ["RELEASING", "FINISHED", "NOT_YET_RELEASED"] as const;
const FORMATS = ["TV", "MOVIE", "OVA", "ONA", "SPECIAL"] as const;
const SORTS = ["POPULARITY_DESC", "SCORE_DESC", "TRENDING_DESC", "START_DATE_DESC", "TITLE_ROMAJI"] as const;
const SEASON_ORDER = ["WINTER", "SPRING", "SUMMER", "FALL"];
const ANY = "__any__";

function seasonOptions(): { season: string; year: number }[] {
  const now = new Date();
  const month = now.getMonth();
  let year = now.getFullYear() + (month === 11 ? 1 : 0);
  let idx = month === 11 || month < 2 ? 0 : month < 5 ? 1 : month < 8 ? 2 : 3;
  idx += 1;
  if (idx > 3) {
    idx = 0;
    year += 1;
  }
  const out = [];
  for (let i = 0; i < 4 * 8; i++) {
    out.push({ season: SEASON_ORDER[idx], year });
    idx -= 1;
    if (idx < 0) {
      idx = 3;
      year -= 1;
    }
  }
  return out;
}

function Chip({ active, onClick, children, tone = "accent" }: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  tone?: "accent" | "accent2";
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "rounded-full px-[11px] py-1.5 text-[11.5px] font-semibold transition-colors",
        active
          ? tone === "accent"
            ? "bg-primary text-[#0B0C10]"
            : "bg-accent2 text-[#0B0C10]"
          : "border border-[#262A35] text-[#B5B9C4] hover:border-[#33374A] hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}

function FilterSection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2.5">
      <span className="text-[11px] font-bold tracking-[0.05em] text-[#6C7180] uppercase">{label}</span>
      {children}
    </div>
  );
}

function StatusBadge({ status }: { status: string | null }) {
  if (!status || status === "CANCELLED" || status === "HIATUS") return null;
  const label = statusLabel(status).toUpperCase();
  const style =
    status === "RELEASING"
      ? "bg-accent2 text-[#0B0C10]"
      : status === "NOT_YET_RELEASED"
        ? "border border-white/50 bg-transparent px-1.5 py-0.5 text-[#EEF0F4]"
        : "bg-black/40 text-[#D7D9DE]";
  return (
    <span className={cn("absolute top-2 left-2 rounded-[5px] px-[7px] py-[3px] text-[9px] font-bold tracking-[0.02em]", style)}>
      {label}
    </span>
  );
}

function ExploreCard({ anime, onOpen, onAdd }: { anime: AnimeSummary; onOpen: () => void; onAdd: () => void }) {
  const { t } = useTranslation();
  const meta =
    anime.status === "NOT_YET_RELEASED" && anime.start_year
      ? anime.start_month
        ? new Date(anime.start_year, anime.start_month - 1).toLocaleDateString(currentLocale(), {
            month: "short",
            year: "numeric",
          })
        : String(anime.start_year)
      : [anime.start_year ?? anime.season_year, anime.format && tDynamic(`explore.formats.${anime.format}`, anime.format)]
          .filter(Boolean)
          .join(" · ");
  return (
    <article className="flex w-[168px] flex-col gap-2">
      <div
        role="button"
        tabIndex={0}
        onClick={onOpen}
        onKeyDown={(e) => e.key === "Enter" && onOpen()}
        className="group relative h-[236px] w-[168px] cursor-pointer overflow-hidden rounded-xl bg-secondary ring-1 ring-white/5 transition-shadow hover:ring-white/15"
      >
        {anime.cover_url && (
          <img
            src={anime.cover_url}
            alt=""
            loading="lazy"
            className="absolute inset-0 size-full object-cover transition-transform duration-300 group-hover:scale-[1.04]"
          />
        )}
        <StatusBadge status={anime.status} />
        <button
          type="button"
          aria-label={t("common.addToLibrary")}
          title={t("common.addToLibrary")}
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="absolute top-2 right-2 flex size-[26px] items-center justify-center rounded-full bg-black/45 text-white transition-colors hover:bg-primary hover:text-[#0B0C10]"
        >
          <Plus className="size-[13px]" strokeWidth={2.4} />
        </button>
      </div>
      <h3 className="truncate text-[13.5px] leading-[1.3] font-semibold" title={anime.title}>
        {anime.title}
      </h3>
      <div className="flex items-center justify-between gap-2">
        {anime.score != null ? (
          <span className="flex items-center gap-1 text-[11px] text-[#B5B9C4]">
            <Star className="size-[11px] fill-accent2 text-accent2" />
            {(anime.score / 10).toFixed(1)}
          </span>
        ) : (
          <span className="text-[11px] text-[#4E5361]">{t("explore.noScore")}</span>
        )}
        <span className="truncate text-[11px] text-[#6C7180]">{meta}</span>
      </div>
    </article>
  );
}

const EMPTY: CatalogFilter = { genres: [], formats: [], sort: "POPULARITY_DESC" };

export default function Explore() {
  const { t } = useTranslation();
  const term = useSearchStore((s) => s.term);
  const [filter, setFilter] = useState<CatalogFilter>(EMPTY);
  const [detailTarget, setDetailTarget] = useState<AnimeSummary | null>(null);
  const [addTarget, setAddTarget] = useState<AnimeSummary | null>(null);
  const sentinel = useRef<HTMLDivElement>(null);
  const seasons = useMemo(seasonOptions, []);

  useEffect(() => {
    const id = setTimeout(() => setFilter((f) => ({ ...f, search: term.trim() || undefined })), 400);
    return () => clearTimeout(id);
  }, [term]);

  const query = useInfiniteQuery({
    queryKey: ["explore", filter],
    queryFn: ({ pageParam }) => anilistCatalog(filter, pageParam),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.has_next ? pages.length + 1 : undefined),
    staleTime: 5 * 60_000,
  });

  const items = useMemo(() => {
    const seen = new Set<number>();
    return (query.data?.pages ?? [])
      .flatMap((p) => p.items)
      .filter((a) => (seen.has(a.anilist_id) ? false : (seen.add(a.anilist_id), true)));
  }, [query.data]);
  const total = query.data?.pages[0]?.total ?? null;

  useEffect(() => {
    const el = sentinel.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting && query.hasNextPage && !query.isFetchingNextPage) query.fetchNextPage();
      },
      { rootMargin: "600px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

  const set = (patch: Partial<CatalogFilter>) => setFilter((f) => ({ ...f, ...patch }));
  const toggle = (list: string[], value: string) =>
    list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
  const stars = filter.min_score != null ? Math.round((filter.min_score + 1) / 20) : 0;
  const seasonValue = filter.season && filter.year ? `${filter.season}:${filter.year}` : ANY;
  const hasFilters =
    filter.genres.length > 0 ||
    filter.formats.length > 0 ||
    !!filter.status ||
    !!filter.season ||
    filter.min_score != null;

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-baseline justify-between">
        <div className="flex items-baseline gap-3">
          <h1 className="text-[26px] font-bold">{t("explore.title")}</h1>
          {total != null && (
            <span className="text-[13px] text-[#6C7180]">
              {t("explore.titlesCount", { count: total, formatted: total.toLocaleString(currentLocale()) })}
            </span>
          )}
        </div>
        <Select
          value={filter.sort ?? "POPULARITY_DESC"}
          onValueChange={(sort) => set({ sort })}
        >
          <SelectTrigger
            size="sm"
            className="h-auto gap-1.5 rounded-lg border-[#21242C] bg-[#15171D] px-3 py-2 text-xs text-[#B5B9C4]"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent align="end">
            {SORTS.map((s) => (
              <SelectItem key={s} value={s}>
                {t(`explore.sort.${s}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="flex items-start gap-7">
        <nav aria-label={t("explore.filters")} className="sticky top-0 flex w-[232px] shrink-0 flex-col gap-[22px]">
          <div className="flex items-center justify-between">
            <h2 className="text-[15px] font-bold">{t("explore.filters")}</h2>
            {hasFilters && (
              <button
                type="button"
                onClick={() => setFilter((f) => ({ ...EMPTY, search: f.search, sort: f.sort }))}
                className="text-xs font-semibold text-primary"
              >
                {t("explore.clear")}
              </button>
            )}
          </div>

          <FilterSection label={t("explore.genre")}>
            <div className="flex flex-wrap gap-1.5">
              {GENRES.map((g) => (
                <Chip key={g} active={filter.genres.includes(g)} onClick={() => set({ genres: toggle(filter.genres, g) })}>
                  {genreLabel(g)}
                </Chip>
              ))}
            </div>
          </FilterSection>

          <div className="h-px bg-[#1B1E27]" />

          <FilterSection label={t("explore.status")}>
            <div className="flex flex-wrap gap-1.5">
              {STATUSES.map((s) => (
                <Chip
                  key={s}
                  tone="accent2"
                  active={filter.status === s}
                  onClick={() => set({ status: filter.status === s ? undefined : s })}
                >
                  {statusLabel(s)}
                </Chip>
              ))}
            </div>
          </FilterSection>

          <div className="h-px bg-[#1B1E27]" />

          <FilterSection label={t("explore.format")}>
            <div className="flex flex-wrap gap-1.5">
              {FORMATS.map((f) => (
                <Chip key={f} active={filter.formats.includes(f)} onClick={() => set({ formats: toggle(filter.formats, f) })}>
                  {t(`explore.formats.${f}`)}
                </Chip>
              ))}
            </div>
          </FilterSection>

          <div className="h-px bg-[#1B1E27]" />

          <FilterSection label={t("explore.season")}>
            <Select
              value={seasonValue}
              onValueChange={(v) => {
                if (v === ANY) return set({ season: undefined, year: undefined });
                const [season, year] = v.split(":");
                set({ season, year: Number(year) });
              }}
            >
              <SelectTrigger className="h-auto w-full rounded-lg border-[#21242C] bg-[#15171D] px-3 py-[9px] text-xs text-foreground">
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="max-h-72">
                <SelectItem value={ANY}>{t("explore.anySeason")}</SelectItem>
                {seasons.map(({ season, year }) => (
                  <SelectItem key={`${season}:${year}`} value={`${season}:${year}`}>
                    {seasonLabel(season)} {year}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FilterSection>

          <div className="h-px bg-[#1B1E27]" />

          <FilterSection label={t("explore.minScore")}>
            <div className="flex items-center gap-1.5">
              {[1, 2, 3, 4, 5].map((n) => (
                <button
                  key={n}
                  type="button"
                  aria-label={`${n}`}
                  onClick={() => set({ min_score: stars === n ? undefined : n * 20 - 1 })}
                >
                  <Star
                    className={cn("size-4", n <= stars ? "fill-accent2 text-accent2" : "text-[#33374A]")}
                    strokeWidth={1.5}
                  />
                </button>
              ))}
              <span className="ml-1 text-xs text-[#6C7180]">
                {stars > 0 ? `${stars.toLocaleString(currentLocale(), { minimumFractionDigits: 1 })}+` : t("explore.anyScore")}
              </span>
            </div>
          </FilterSection>
        </nav>

        <div className="flex min-w-0 flex-1 flex-col gap-4">
          {total != null && !query.isLoading && (
            <p className="text-[12.5px] text-[#6C7180]">
              {t("explore.results", { count: total, formatted: total.toLocaleString(currentLocale()) })}
            </p>
          )}
          {query.isError ? (
            <p className="text-sm text-destructive">{t("explore.error")}</p>
          ) : query.isLoading ? (
            <div className="flex justify-center py-16">
              <Loader2 className="size-7 animate-spin text-muted-foreground" />
            </div>
          ) : items.length === 0 ? (
            <p className="py-16 text-center text-sm text-muted-foreground">{t("explore.empty")}</p>
          ) : (
            <div className="flex flex-wrap gap-x-[18px] gap-y-5">
              {items.map((anime) => (
                <ExploreCard
                  key={anime.anilist_id}
                  anime={anime}
                  onOpen={() => setDetailTarget(anime)}
                  onAdd={() => setAddTarget(anime)}
                />
              ))}
            </div>
          )}
          <div ref={sentinel} className="flex h-10 items-center justify-center">
            {query.isFetchingNextPage && <Loader2 className="size-5 animate-spin text-muted-foreground" />}
          </div>
        </div>
      </div>

      <AnimeDetailModal
        anime={detailTarget}
        onOpenChange={(open) => !open && setDetailTarget(null)}
        onAdd={(anime) => {
          setDetailTarget(null);
          setAddTarget(anime);
        }}
      />
      <WatchFormModal anime={addTarget} onOpenChange={(open) => !open && setAddTarget(null)} />
    </div>
  );
}
