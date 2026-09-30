import { useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { UpdateBanner } from "@/components/layout/UpdateBanner";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "framer-motion";
import { Download, Home as HomeIcon, Library, Search, Settings } from "lucide-react";
import { cn } from "@/lib/utils";
import { TopBar } from "./TopBar";
import { Onboarding } from "./Onboarding";

const NAV_ITEMS = [
  { to: "/", labelKey: "nav.home", end: true, icon: HomeIcon },
  { to: "/library", labelKey: "nav.library", end: false, icon: Library },
  { to: "/downloads", labelKey: "nav.downloads", end: false, icon: Download },
] as const;

// Rendered in a portal: the Explore button lives inside an overflow-hidden animation wrapper.
function SideTooltip({ label, children }: { label: string; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  return (
    <div
      ref={ref}
      onMouseEnter={() => {
        const r = ref.current?.getBoundingClientRect();
        if (r) setPos({ top: r.top + r.height / 2, left: r.right + 10 });
      }}
      onMouseLeave={() => setPos(null)}
    >
      {children}
      {pos &&
        createPortal(
          <div
            role="tooltip"
            style={{ top: pos.top, left: pos.left }}
            className="pointer-events-none fixed z-[100] -translate-y-1/2 rounded-md border border-[#262A35] bg-[#1B1E27] px-2.5 py-1.5 text-xs font-semibold whitespace-nowrap text-foreground shadow-[0_6px_20px_rgba(0,0,0,0.45)]"
          >
            {label}
          </div>,
          document.body,
        )}
    </div>
  );
}

function NavButton({
  to,
  label,
  icon: Icon,
  end,
  small,
  parentActive,
  tour,
}: {
  to: string;
  label: string;
  icon: typeof HomeIcon;
  end?: boolean;
  small?: boolean;
  parentActive?: boolean;
  tour?: string;
}) {
  return (
    <SideTooltip label={label}>
      <NavLink
        to={to}
        end={end}
        aria-label={label}
        data-tour={tour}
        className={({ isActive }) =>
          cn(
            "flex items-center justify-center rounded-xl transition-colors",
            small ? "size-10" : "size-12",
            isActive
              ? "bg-secondary text-primary"
              : parentActive
                ? "text-primary"
                : "text-muted-foreground hover:text-foreground",
          )
        }
      >
        <Icon className={small ? "size-[18px]" : "size-[22px]"} strokeWidth={1.8} />
      </NavLink>
    </SideTooltip>
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
          {NAV_ITEMS.map((item) =>
            item.to === "/" ? (
              <div key={item.to} className="flex flex-col items-center">
                <NavButton
                  to={item.to}
                  end={item.end}
                  icon={item.icon}
                  label={t(item.labelKey)}
                  tour="nav-home"
                />
                <AnimatePresence initial={false}>
                  {(location.pathname === "/" || location.pathname === "/explore") && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.18 }}
                      className="flex flex-col items-center overflow-hidden pt-2"
                    >
                      <NavButton to="/explore" icon={Search} label={t("nav.explore")} tour="nav-explore" />
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            ) : (
              <NavButton
                key={item.to}
                to={item.to}
                end={item.end}
                icon={item.icon}
                label={t(item.labelKey)}
                tour={item.to === "/library" ? "nav-library" : "nav-downloads"}
              />
            ),
          )}
        </div>

        <div className="mt-auto flex flex-col items-center gap-5">
          <SideTooltip label={t("nav.settings")}>
            <NavLink
              to="/config"
              aria-label={t("nav.settings")}
              data-tour="nav-settings"
              className={cn(
                "flex size-11 items-center justify-center rounded-xl transition-colors",
                location.pathname === "/config"
                  ? "bg-secondary text-primary"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Settings className="size-[21px]" strokeWidth={1.8} />
            </NavLink>
          </SideTooltip>
        </div>
      </nav>

      <Onboarding />
      <div className="flex flex-1 flex-col overflow-hidden">
        <TopBar />
        <main className="flex-1 overflow-y-auto px-8 py-7">
          <div className="flex h-full flex-col gap-9">
            <Outlet />
            <UpdateBanner />
          </div>
        </main>
      </div>
    </div>
  );
}
