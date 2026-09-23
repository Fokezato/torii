import { useTranslation } from "react-i18next";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

/// Valor do filtro "todos os gêneros" (o rótulo vem da tradução).
export const ALL_GENRES = "__all__";

interface GenreFilterBarProps {
  genres: string[];
  value: string;
  onChange: (genre: string) => void;
}

export function GenreFilterBar({ genres, value, onChange }: GenreFilterBarProps) {
  const { t } = useTranslation();
  const options = [ALL_GENRES, ...genres];

  return (
    <ScrollArea className="w-full whitespace-nowrap">
      <div className="flex gap-2 pb-1">
        {options.map((g) => (
          <button
            key={g}
            type="button"
            onClick={() => onChange(g)}
            className={cn(
              "shrink-0 rounded-full px-3.5 py-1.5 text-[12.5px] font-semibold transition-colors",
              value === g
                ? "bg-primary text-primary-foreground"
                : "bg-secondary text-secondary-foreground hover:bg-secondary/70",
            )}
          >
            {g === ALL_GENRES ? t("home.allGenres") : g}
          </button>
        ))}
      </div>
      <ScrollBar orientation="horizontal" />
    </ScrollArea>
  );
}
