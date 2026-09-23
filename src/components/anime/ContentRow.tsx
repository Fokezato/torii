import type { ReactNode } from "react";
import { AnimeCard } from "./AnimeCard";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import type { AnimeSummary } from "@/lib/anilist";

interface ContentRowProps {
  title: string;
  subtitle?: string;
  items?: AnimeSummary[];
  isLoading?: boolean;
  onSelect?: (anime: AnimeSummary) => void;
  extra?: ReactNode;
}

export function ContentRow({ title, subtitle, items, isLoading, onSelect, extra }: ContentRowProps) {
  return (
    <section className="space-y-4">
      <div className="flex items-baseline gap-3">
        <h2 className="text-[22px] font-bold tracking-tight">{title}</h2>
        {subtitle && <span className="text-[13px] text-[#6C7180]">{subtitle}</span>}
      </div>
      {extra}
      <ScrollArea className="w-full whitespace-nowrap">
        <div className="flex gap-[18px] pb-4">
          {isLoading &&
            Array.from({ length: 8 }).map((_, i) => (
              <Skeleton key={i} className="h-[236px] w-[168px] shrink-0 rounded-xl" />
            ))}
          {!isLoading &&
            items?.map((anime) => (
              <AnimeCard key={anime.anilist_id} anime={anime} onClick={() => onSelect?.(anime)} />
            ))}
          {!isLoading && items?.length === 0 && (
            <p className="py-8 text-sm text-muted-foreground">Nada encontrado.</p>
          )}
        </div>
        <ScrollBar orientation="horizontal" />
      </ScrollArea>
    </section>
  );
}
