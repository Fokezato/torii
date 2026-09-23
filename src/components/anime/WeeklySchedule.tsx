import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listWatches } from "@/lib/watches";
import { getSchedule } from "@/lib/anilist";
import { currentLocale } from "@/i18n";

/// Nomes curtos dos dias (Dom..Sáb / Sun..Sat) no idioma atual, começando
/// no domingo (mesma ordem de `Date.getDay()`).
function weekdayNames(): string[] {
  const format = new Intl.DateTimeFormat(currentLocale(), { weekday: "short" });
  // 2023-01-01 foi um domingo.
  return Array.from({ length: 7 }, (_, i) => format.format(new Date(2023, 0, 1 + i)).replace(".", ""));
}

interface DayEntry {
  title: string;
  cover_url: string | null;
  time: string;
  episode: number;
}

export function WeeklySchedule() {
  const { t } = useTranslation();
  const { data: watches } = useQuery({ queryKey: ["watches"], queryFn: listWatches });

  const ids = useMemo(
    () =>
      (watches ?? [])
        .filter((w) => w.active && w.anilist_id != null)
        .map((w) => w.anilist_id as number),
    [watches],
  );

  const { data: schedule } = useQuery({
    queryKey: ["schedule", ids],
    queryFn: () => getSchedule(ids),
    enabled: ids.length > 0,
  });

  const byDay = useMemo(() => {
    const groups: DayEntry[][] = Array.from({ length: 7 }, () => []);
    for (const anime of schedule ?? []) {
      if (!anime.next_airing_at) continue;
      const date = new Date(anime.next_airing_at * 1000);
      groups[date.getDay()].push({
        title: anime.title,
        cover_url: anime.cover_url,
        time: date.toLocaleTimeString(currentLocale(), { hour: "2-digit", minute: "2-digit" }),
        episode: anime.next_airing_episode ?? 0,
      });
    }
    groups.forEach((g) => g.sort((a, b) => a.time.localeCompare(b.time)));
    return groups;
  }, [schedule]);

  if (ids.length === 0 || !byDay.some((g) => g.length > 0)) return null;

  const today = new Date().getDay();

  return (
    <section className="space-y-3">
      <div className="flex items-center gap-2.5">
        <span className="h-4 w-1 rounded-full bg-primary" />
        <h2 className="text-lg font-semibold tracking-tight">{t("home.schedule")}</h2>
      </div>
      <div className="grid grid-cols-7 gap-2.5">
        {weekdayNames().map((label, i) => (
          <div
            key={label}
            className={`flex min-h-[140px] flex-col gap-2 rounded-[14px] border p-2.5 ${
              i === today ? "border-primary/40 bg-primary/[0.06]" : "border-[#1E212A] bg-muted"
            }`}
          >
            <span
              className={`text-center text-[11px] font-bold tracking-wide uppercase ${
                i === today ? "text-primary" : "text-[#6C7180]"
              }`}
            >
              {label}
            </span>
            <div className="flex flex-1 flex-col gap-1.5">
              {byDay[i].length === 0 ? (
                <span className="mt-2 text-center text-xs text-[#4E5361]">—</span>
              ) : (
                byDay[i].map((entry, idx) => (
                  <div key={idx} className="flex items-center gap-1.5 rounded-lg bg-black/20 p-1.5">
                    <div className="size-7 shrink-0 overflow-hidden rounded bg-secondary">
                      {entry.cover_url && (
                        <img src={entry.cover_url} alt="" className="h-full w-full object-cover" />
                      )}
                    </div>
                    <div className="min-w-0">
                      <p className="line-clamp-1 text-[11px] font-semibold">{entry.title}</p>
                      <p className="text-[10px] text-[#6C7180]">
                        {t("common.episodeShort", { number: entry.episode })} · {entry.time}
                      </p>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
