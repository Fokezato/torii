import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Play, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { listWatches } from "@/lib/watches";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { statusLabel } from "@/lib/constants";
import { timeAgo } from "@/lib/format";

export function MyListRow() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data } = useQuery({ queryKey: ["watches"], queryFn: listWatches });
  const items = (data ?? []).slice(0, 10);

  if (items.length === 0) return null;

  return (
    <section className="space-y-3">
      <div className="flex items-center gap-2.5">
        <span className="h-4 w-1 rounded-full bg-primary" />
        <h2 className="text-lg font-semibold tracking-tight">{t("home.myList")}</h2>
      </div>
      <ScrollArea className="w-full whitespace-nowrap">
        <div className="flex gap-4 pb-4">
          {items.map((w) => (
            <button
              key={w.id}
              type="button"
              onClick={() => navigate(`/library/${w.id}`)}
              className="group relative h-36 w-64 shrink-0 overflow-hidden rounded-xl bg-secondary text-left shadow-[0_4px_16px_rgba(0,0,0,0.4)] ring-1 ring-white/5 transition-shadow hover:shadow-[0_12px_32px_rgba(0,0,0,0.55)] hover:ring-white/15"
            >
              {w.cover_url ? (
                <img src={w.cover_url} alt="" className="absolute inset-0 h-full w-full object-cover" />
              ) : (
                <div className="absolute inset-0 bg-gradient-to-br from-secondary to-muted" />
              )}
              <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/10 to-transparent" />
              <div className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity group-hover:opacity-100">
                <span className="flex size-10 items-center justify-center rounded-full bg-white/90">
                  <Play className="size-4 translate-x-0.5 fill-black text-black" />
                </span>
              </div>

              {w.status && (
                <span
                  className={`absolute top-2 right-2 rounded-md px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide ${
                    w.status === "FINISHED"
                      ? "bg-primary text-primary-foreground"
                      : "bg-black/55 text-white backdrop-blur-sm"
                  }`}
                >
                  {statusLabel(w.status)}
                </span>
              )}

              <div className="absolute inset-x-0 bottom-0 p-3">
                <p className="line-clamp-1 text-sm font-semibold text-white">{w.title}</p>
                <div className="mt-0.5 flex items-center gap-2 text-xs text-white/65">
                  <span>{t("home.updated", { when: timeAgo(w.updated_at) })}</span>
                  {w.rating != null && (
                    <span className="flex items-center gap-0.5">
                      <Star className="size-3 fill-yellow-400 text-yellow-400" />
                      {w.rating}
                    </span>
                  )}
                  {!w.active && <span>· {t("home.paused")}</span>}
                </div>
              </div>
            </button>
          ))}
        </div>
        <ScrollBar orientation="horizontal" />
      </ScrollArea>
    </section>
  );
}
