import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUpCircle, X } from "lucide-react";
import { getSettings } from "@/lib/tauri";
import { useUpdateStore } from "@/stores/update";

// Espera o app assentar antes de ir na rede.
const STARTUP_CHECK_DELAY_MS = 8_000;

/// Cartão no canto quando tem versão nova do Torii (ver `stores/update`).
/// Checa sozinho ao abrir, se "Verificar atualizações automaticamente"
/// estiver ligado em Config > Sobre.
export function UpdateBanner() {
  const { t } = useTranslation();
  const { phase, update, progress, dismissed, checkNow, install, dismiss } = useUpdateStore();

  useEffect(() => {
    const timer = setTimeout(() => {
      getSettings()
        .then((s) => {
          if (s.auto_update_check !== "0") checkNow();
        })
        .catch(() => {});
    }, STARTUP_CHECK_DELAY_MS);
    return () => clearTimeout(timer);
  }, [checkNow]);

  const visible = !dismissed && (phase === "available" || phase === "downloading");
  if (!visible || !update) return null;

  return (
    <div className="fixed right-5 bottom-5 z-50 flex w-[320px] flex-col gap-3 rounded-[14px] border border-[#262A35] bg-[#15171D] p-4 shadow-[0_12px_32px_rgba(0,0,0,0.55)]">
      <div className="flex items-start gap-3">
        <ArrowUpCircle className="mt-0.5 size-5 shrink-0 text-primary" />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="text-[13px] font-semibold">{t("update.available", { version: update.version })}</span>
          <span className="text-xs text-[#6C7180]">{t("update.hint")}</span>
        </div>
        {phase === "available" && (
          <button
            type="button"
            aria-label={t("common.close")}
            onClick={dismiss}
            className="flex size-6 shrink-0 items-center justify-center rounded-md text-[#6C7180] hover:bg-white/5 hover:text-foreground"
          >
            <X className="size-3.5" />
          </button>
        )}
      </div>

      {phase === "downloading" ? (
        <div className="flex flex-col gap-1.5">
          <div className="h-1.5 overflow-hidden rounded-full bg-[#22252E]">
            <div className="h-full rounded-full bg-primary" style={{ width: `${progress}%` }} />
          </div>
          <span className="text-[11px] text-[#6C7180]">{t("update.downloading", { percent: Math.round(progress) })}</span>
        </div>
      ) : (
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={dismiss}
            className="rounded-lg border border-[#33374A] px-3 py-1.5 text-xs font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
          >
            {t("update.later")}
          </button>
          <button
            type="button"
            onClick={() => install()}
            className="rounded-lg bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground transition-opacity hover:opacity-90"
          >
            {t("update.install")}
          </button>
        </div>
      )}
    </div>
  );
}
