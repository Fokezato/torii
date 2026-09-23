import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Download, Home as HomeIcon, Library, Settings } from "lucide-react";
import { cn } from "@/lib/utils";
import { TopBar } from "./TopBar";

const NAV_ITEMS = [
  { to: "/", labelKey: "nav.home", end: true, icon: HomeIcon },
  { to: "/library", labelKey: "nav.library", end: false, icon: Library },
  { to: "/downloads", labelKey: "nav.downloads", end: false, icon: Download },
] as const;

function NavButton({
  to,
  label,
  icon: Icon,
  end,
}: {
  to: string;
  label: string;
  icon: typeof HomeIcon;
  end?: boolean;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      aria-label={label}
      title={label}
      className={({ isActive }) =>
        cn(
          "flex size-12 items-center justify-center rounded-xl transition-colors",
          isActive ? "bg-secondary text-primary" : "text-muted-foreground hover:text-foreground",
        )
      }
    >
      <Icon className="size-[22px]" strokeWidth={1.8} />
    </NavLink>
  );
}

export function AppShell() {
  const location = useLocation();
  const { t } = useTranslation();

  return (
    <div className="flex h-screen bg-background text-foreground">
      <nav
        aria-label={t("nav.main")}
        className="flex w-[88px] shrink-0 flex-col items-center gap-9 border-r border-border bg-sidebar py-6"
      >
        <span
          aria-label="Torii"
          className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-primary text-primary-foreground"
        >
          <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
            <path
              fillRule="evenodd"
              clipRule="evenodd"
              d="M2.222 3.372a1 1 0 0 1 1.027-.34c1.078.277 2.167.517 3.257.738c1.852.375 4.015.73 5.494.73s3.642-.355 5.494-.73c1.09-.22 2.178-.461 3.256-.738a1 1 0 0 1 1.144 1.415l-2 4c-.15.3-.446.507-.778.546A91 91 0 0 1 16 9.3v1.366a58 58 0 0 0 3.797-.644a1 1 0 0 1 .406 1.958q-.6.123-1.203.23V18a1 1 0 1 1 0 2h-5a1 1 0 1 1 0-2v-5.095c-.692.059-1.374.095-2 .095s-1.308-.037-2-.095V18a1 1 0 1 1 0 2H5a1 1 0 0 1 0-2v-5.79a49 49 0 0 1-1.203-.23a1 1 0 0 1 .406-1.96l.003.002q1.886.382 3.794.643V9.299a91 91 0 0 1-3.116-.306q-.001 0 0 0a1 1 0 0 1-.778-.546l-2-4a1 1 0 0 1 .116-1.075M12 9.5c.617 0 1.304-.024 2-.062v1.459c-.703.063-1.387.103-2 .103s-1.297-.04-2-.103v-1.46c.696.039 1.383.063 2 .063"
            />
          </svg>
        </span>

        <div className="flex flex-col items-center gap-2">
          {NAV_ITEMS.map((item) => (
            <NavButton key={item.to} to={item.to} end={item.end} icon={item.icon} label={t(item.labelKey)} />
          ))}
        </div>

        <div className="mt-auto flex flex-col items-center gap-5">
          <NavLink
            to="/config"
            aria-label={t("nav.settings")}
            title={t("nav.settings")}
            className={cn(
              "flex size-11 items-center justify-center rounded-xl transition-colors",
              location.pathname === "/config"
                ? "bg-secondary text-primary"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Settings className="size-[21px]" strokeWidth={1.8} />
          </NavLink>
        </div>
      </nav>

      <div className="flex flex-1 flex-col overflow-hidden">
        <TopBar />
        <main className="flex-1 overflow-y-auto px-8 py-7">
          {/* h-full: rota que precisa preencher a altura disponível sem
              scroll (ex. player de vídeo) tem em que basear o próprio
              h-full — sem isso o wrapper crescia só com o conteúdo (auto),
              não tinha altura definida pra nada herdar. Scroll de página
              longa continua funcionando normal: quem rola é o <main> (via
              overflow-y-auto), não esse wrapper — h-full aqui não clipa
              nada, só dá uma base de altura real pra quem quiser usar. */}
          <div className="flex h-full flex-col gap-9">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
