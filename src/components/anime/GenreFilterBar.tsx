import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

interface GenreFilterBarProps {
  genres: string[];
  value: string;
  onChange: (genre: string) => void;
}

export function GenreFilterBar({ genres, value, onChange }: GenreFilterBarProps) {
  const options = ["Todos", ...genres];

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
            {g}
          </button>
        ))}
      </div>
      <ScrollBar orientation="horizontal" />
    </ScrollArea>
  );
}
