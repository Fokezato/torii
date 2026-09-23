import { useQuery } from "@tanstack/react-query";
import { Activity } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { getActivityLog } from "@/lib/activity";
import { timeAgo } from "@/lib/format";

export function ActivityStatus() {
  const { t } = useTranslation();
  const { data } = useQuery({
    queryKey: ["activity-log"],
    queryFn: getActivityLog,
    refetchInterval: 5000,
  });

  const latest = data?.[0];

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex max-w-[280px] items-center gap-2 rounded-full border border-border bg-muted px-3.5 py-2 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
        >
          <Activity className="size-[15px] shrink-0 text-accent2" strokeWidth={2} />
          <span className="truncate">{latest ? latest.message : t("activity.none")}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 max-h-96 overflow-y-auto p-2">
        <p className="px-1.5 pt-1 pb-2 text-[11px] font-bold tracking-wide text-muted-foreground uppercase">
          {t("activity.recent")}
        </p>
        {!data || data.length === 0 ? (
          <p className="px-1.5 py-3 text-xs text-muted-foreground">{t("activity.empty")}</p>
        ) : (
          <div className="flex flex-col">
            {data.map((entry) => (
              <div key={entry.id} className="border-b border-border/60 px-1.5 py-2 last:border-0">
                <p className={`text-xs ${entry.level === "error" ? "text-destructive" : "text-foreground"}`}>
                  {entry.message}
                </p>
                <p className="mt-0.5 text-[11px] text-muted-foreground">{timeAgo(entry.timestamp)}</p>
              </div>
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
