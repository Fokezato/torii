import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { browseSeason, browseTrending, anilistSearch, type AnimeSummary } from "@/lib/anilist";
import { HeroCarousel } from "@/components/anime/HeroCarousel";
import { ContentRow } from "@/components/anime/ContentRow";
import { GenreFilterBar } from "@/components/anime/GenreFilterBar";
import { MyListRow } from "@/components/anime/MyListRow";
import { WeeklySchedule } from "@/components/anime/WeeklySchedule";
import { NewsSection } from "@/components/anime/NewsSection";
import { AnimeDetailModal } from "@/components/anime/AnimeDetailModal";
import { WatchFormModal } from "@/components/anime/WatchFormModal";
import { useSearchStore } from "@/stores/search";

export default function Home() {
  const term = useSearchStore((s) => s.term);
  const [detailTarget, setDetailTarget] = useState<AnimeSummary | null>(null);
  const [addTarget, setAddTarget] = useState<AnimeSummary | null>(null);
  const [genre, setGenre] = useState("Todos");
  const isSearching = term.trim().length > 0;

  const season = useQuery({ queryKey: ["season"], queryFn: () => browseSeason() });
  const trending = useQuery({ queryKey: ["trending"], queryFn: () => browseTrending() });
  const search = useQuery({
    queryKey: ["search", term],
    queryFn: () => anilistSearch(term),
    enabled: isSearching,
  });

  const heroItems = useMemo(() => {
    const source = trending.data?.length ? trending.data : (season.data ?? []);
    return source.slice(0, 6);
  }, [trending.data, season.data]);

  const seasonGenres = useMemo(() => {
    const set = new Set<string>();
    season.data?.forEach((a) => a.genres.forEach((g) => set.add(g)));
    return Array.from(set).sort();
  }, [season.data]);

  const filteredSeason = useMemo(() => {
    if (genre === "Todos") return season.data;
    return season.data?.filter((a) => a.genres.includes(genre));
  }, [season.data, genre]);

  return (
    <>
      {!isSearching && <HeroCarousel items={heroItems} onAdd={setAddTarget} />}

      {isSearching ? (
        <ContentRow
          title={`Resultados para "${term}"`}
          items={search.data}
          isLoading={search.isLoading}
          onSelect={setDetailTarget}
        />
      ) : (
        <>
          <ContentRow
            title="Temporada atual"
            subtitle={filteredSeason ? `${filteredSeason.length} títulos` : undefined}
            items={filteredSeason}
            isLoading={season.isLoading}
            onSelect={setDetailTarget}
            extra={
              seasonGenres.length > 0 && (
                <GenreFilterBar genres={seasonGenres} value={genre} onChange={setGenre} />
              )
            }
          />
          <ContentRow
            title="Em alta"
            items={trending.data}
            isLoading={trending.isLoading}
            onSelect={setDetailTarget}
          />
          <MyListRow />
          <WeeklySchedule />
          <NewsSection />
        </>
      )}

      <AnimeDetailModal
        anime={detailTarget}
        onOpenChange={(open) => !open && setDetailTarget(null)}
        onAdd={(anime) => {
          setDetailTarget(null);
          setAddTarget(anime);
        }}
      />
      <WatchFormModal anime={addTarget} onOpenChange={(open) => !open && setAddTarget(null)} />
    </>
  );
}
