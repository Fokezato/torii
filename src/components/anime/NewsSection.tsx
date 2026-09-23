import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import { getAnimeNews } from "@/lib/news";
import { Skeleton } from "@/components/ui/skeleton";
import { timeAgo } from "@/lib/format";
import { TranslatedText } from "@/components/shared/TranslatedText";

export function NewsSection() {
  const { t } = useTranslation();
  const { data, isLoading } = useQuery({
    queryKey: ["anime-news"],
    queryFn: getAnimeNews,
    staleTime: 15 * 60 * 1000,
    retry: 1,
  });

  if (!isLoading && (!data || data.length === 0)) return null;

  return (
    <section className="space-y-4 pb-2">
      <div className="flex items-baseline gap-3">
        <div className="flex items-center gap-2.5">
          <span className="h-4 w-1 rounded-full bg-primary" />
          <h2 className="text-[22px] font-bold tracking-tight">{t("home.news")}</h2>
        </div>
        <span className="text-[13px] text-[#6C7180]">{t("home.newsSource")}</span>
      </div>

      {isLoading ? (
        <div className="grid gap-2 sm:grid-cols-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-[86px] rounded-xl" />
          ))}
        </div>
      ) : (
        <div className="grid gap-2.5 sm:grid-cols-2">
          {data!.map((item) => (
            <button
              key={item.link}
              type="button"
              onClick={() => openUrl(item.link)}
              className="group flex flex-col gap-1.5 rounded-xl border border-border bg-card p-4 text-left transition-colors hover:border-primary/40"
            >
              <div className="flex items-center gap-2 text-[11px] text-[#6C7180]">
                {item.category && (
                  <TranslatedText
                    as="span"
                    text={item.category}
                    className="rounded bg-secondary px-1.5 py-0.5 font-medium text-muted-foreground"
                  />
                )}
                <span>{timeAgo(item.published_at)}</span>
              </div>
              <TranslatedText
                as="h3"
                text={item.title}
                className="line-clamp-2 text-sm leading-snug font-semibold group-hover:text-primary"
              />
              {item.summary && (
                <TranslatedText text={item.summary} className="line-clamp-1 text-xs text-muted-foreground" />
              )}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
