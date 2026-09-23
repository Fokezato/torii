import { useEffect, useState } from "react";
import { Plus, Star } from "lucide-react";
import { Carousel, CarouselContent, CarouselItem, type CarouselApi } from "@/components/ui/carousel";
import type { AnimeSummary } from "@/lib/anilist";
import { STATUS_LABEL } from "@/lib/constants";
import { cn } from "@/lib/utils";

interface HeroCarouselProps {
  items: AnimeSummary[];
  onAdd?: (anime: AnimeSummary) => void;
}

export function HeroCarousel({ items, onAdd }: HeroCarouselProps) {
  const [api, setApi] = useState<CarouselApi>();
  const [current, setCurrent] = useState(0);

  useEffect(() => {
    if (!api) return;
    const onSelect = () => setCurrent(api.selectedScrollSnap());
    onSelect();
    api.on("select", onSelect);
    const id = setInterval(() => api.scrollNext(), 7000);
    return () => {
      clearInterval(id);
      api.off("select", onSelect);
    };
  }, [api]);

  if (items.length === 0) return null;

  return (
    <Carousel setApi={setApi} opts={{ loop: true }} className="w-full">
      <CarouselContent className="ml-0">
        {items.map((anime) => (
          <CarouselItem key={anime.anilist_id} className="pl-0">
            <div className="relative h-[300px] overflow-hidden rounded-[18px] bg-card">
              <div
                aria-hidden
                className="absolute -right-16 -top-20 size-[360px] rounded-full bg-primary opacity-[0.14]"
              />
              <div
                aria-hidden
                className="absolute -bottom-32 right-40 size-[260px] rounded-full bg-accent2 opacity-10"
              />
              {(anime.banner_url ?? anime.cover_url) && (
                <img
                  src={anime.banner_url ?? anime.cover_url ?? undefined}
                  alt=""
                  className="absolute inset-0 h-full w-full object-cover opacity-25"
                />
              )}
              <div className="absolute inset-0 bg-gradient-to-t from-card via-card/70 to-transparent" />

              {items.length > 1 && (
                <div className="absolute right-8 top-8 flex gap-1.5">
                  {items.map((it, i) => (
                    <button
                      key={it.anilist_id}
                      type="button"
                      aria-label={`Slide ${i + 1}`}
                      onClick={() => api?.scrollTo(i)}
                      className={cn(
                        "h-1.5 rounded-full transition-all",
                        i === current ? "w-6 bg-primary" : "w-1.5 bg-white/25 hover:bg-white/45",
                      )}
                    />
                  ))}
                </div>
              )}

              <div className="relative flex h-full max-w-xl flex-col justify-end gap-3.5 px-9 pb-8">
                {anime.status && (
                  <span className="self-start rounded-md bg-accent2 px-2.5 py-1 text-[11px] font-bold tracking-wide text-background uppercase">
                    {STATUS_LABEL[anime.status] ?? anime.status}
                    {anime.episodes ? ` · ${anime.episodes} eps` : ""}
                  </span>
                )}
                <h1 className="text-4xl leading-[1.05] font-bold tracking-tight">{anime.title}</h1>
                {anime.description && (
                  <p className="line-clamp-2 max-w-lg text-sm leading-relaxed text-muted-foreground">
                    {anime.description}
                  </p>
                )}
                <div className="flex flex-wrap items-center gap-4 text-xs text-white/50">
                  {anime.studio && <span>{anime.studio}</span>}
                  {anime.duration && (
                    <>
                      <span aria-hidden>·</span>
                      <span>{anime.duration} min</span>
                    </>
                  )}
                  {anime.score != null && (
                    <>
                      <span aria-hidden>·</span>
                      <span className="flex items-center gap-1">
                        <Star className="size-[13px] fill-accent2 text-accent2" />
                        {(anime.score / 10).toFixed(1)}
                      </span>
                    </>
                  )}
                </div>
                <div className="flex items-center gap-3 pt-1">
                  <button
                    type="button"
                    onClick={() => onAdd?.(anime)}
                    className="flex items-center gap-2 rounded-[10px] bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90"
                  >
                    <Plus className="size-4" strokeWidth={2.2} />
                    Adicionar à biblioteca
                  </button>
                </div>
              </div>
            </div>
          </CarouselItem>
        ))}
      </CarouselContent>
    </Carousel>
  );
}
