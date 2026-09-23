import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import { Search, X } from "lucide-react";
import { useSearchStore } from "@/stores/search";
import { Button } from "@/components/ui/button";
import { ActivityStatus } from "./ActivityStatus";

const PLACEHOLDER_BY_ROUTE: Record<string, string> = {
  "/": "Buscar animes, estúdios...",
  "/library": "Buscar na sua biblioteca...",
};

export function TopBar() {
  const { term, setTerm } = useSearchStore();
  const location = useLocation();

  // Cada página usa o termo com um sentido diferente (busca de anime na
  // Home, filtro na Biblioteca) — carregar texto de uma página pra outra
  // confundiria mais do que ajudaria, então zera ao trocar de rota.
  useEffect(() => {
    setTerm("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname]);

  const placeholder = PLACEHOLDER_BY_ROUTE[location.pathname];
  const showSearch = placeholder != null;

  return (
    <header className="grid h-[72px] shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-border px-8">
      <div />

      {showSearch && (
        <div className="flex h-10 w-[360px] items-center gap-2.5 rounded-[10px] border border-border bg-muted px-3.5 justify-self-center">
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
      )}
      {!showSearch && <div />}

      <div className="justify-self-end">
        <ActivityStatus />
      </div>
    </header>
  );
}
