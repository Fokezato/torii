import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Compass, Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSearchStore } from "@/stores/search";
import { Button } from "@/components/ui/button";
import { ActivityStatus } from "./ActivityStatus";

const PLACEHOLDER_BY_ROUTE = {
  "/": "search.home",
  "/library": "search.library",
  "/explore": "search.home",
} as const;

export function TopBar() {
  const { term, setTerm } = useSearchStore();
  const location = useLocation();
  const navigate = useNavigate();
  const { t } = useTranslation();

  useEffect(() => {
    setTerm("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname]);

  const placeholderKey = PLACEHOLDER_BY_ROUTE[location.pathname as keyof typeof PLACEHOLDER_BY_ROUTE];
  const placeholder = placeholderKey ? t(placeholderKey) : undefined;
  const showSearch = placeholder != null;

  return (
    <header className="grid h-[72px] shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-border px-8">
      <div />

      {showSearch && (
        <div className="flex items-center gap-2 justify-self-center">
        <div className="flex h-10 w-[360px] items-center gap-2.5 rounded-[10px] border border-border bg-muted px-3.5">
          <Search className="size-[17px] shrink-0 text-muted-foreground" strokeWidth={1.8} />
          <input
            value={term}
            onChange={(e) => setTerm(e.target.value)}
            placeholder={placeholder}
            className="w-full bg-transparent text-[13px] text-foreground placeholder:text-muted-foreground focus:outline-none"
          />
          {term && (
            <Button variant="ghost" size="icon-sm" className="-mr-1.5 rounded-full" onClick={() => setTerm("")}>
              <X className="size-3.5" />
            </Button>
          )}
        </div>
        {location.pathname === "/" && (
          <button
            type="button"
            onClick={() => navigate("/explore")}
            title={t("nav.explore")}
            className="flex h-10 items-center gap-2 rounded-[10px] border border-border bg-muted px-3.5 text-[13px] font-medium text-muted-foreground transition-colors hover:text-foreground"
          >
            <Compass className="size-[17px]" strokeWidth={1.8} />
            {t("nav.explore")}
          </button>
        )}
        </div>
      )}
      {!showSearch && <div />}

      <div className="justify-self-end">
        <ActivityStatus />
      </div>
    </header>
  );
}
