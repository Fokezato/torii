import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getSettings, updateSettings } from "@/lib/tauri";
import { broadcastLanguage } from "@/i18n";
import { START_TOUR_EVENT, startTour } from "@/lib/tour";

function Field({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-0.5">
        <span className="text-[13px] font-semibold">{label}</span>
        <span className="text-[11.5px] text-[#6C7180]">{hint}</span>
      </div>
      {children}
    </div>
  );
}

/// First run: welcome with the essential settings, then the guided tour.
export function Onboarding() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: getSettings });
  const [dismissed, setDismissed] = useState(false);
  const open_ = !!settings && settings.onboarding_done !== "1" && !dismissed;

  const save = async (values: Record<string, string>) => {
    await updateSettings(values).catch(() => {});
    queryClient.invalidateQueries({ queryKey: ["settings"] });
  };

  const finish = () => save({ onboarding_done: "1" });

  const runTour = () => {
    navigate("/");
    // Home's anchors need a moment to mount after navigating.
    setTimeout(() => startTour(finish), 400);
  };

  useEffect(() => {
    const onRequest = () => runTour();
    window.addEventListener(START_TOUR_EVENT, onRequest);
    return () => window.removeEventListener(START_TOUR_EVENT, onRequest);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function pickFolder() {
    const result = await open({ directory: true, defaultPath: settings?.library_root || undefined });
    if (typeof result === "string") save({ library_root: result });
  }

  const trigger = { backgroundColor: "#1B1E27", borderColor: "#262A35" };

  return (
    <Dialog open={open_} onOpenChange={() => {}}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "520px", width: "92vw" }}
        className="gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
        onEscapeKeyDown={(e) => e.preventDefault()}
        onInteractOutside={(e) => e.preventDefault()}
      >
        <div className="flex flex-col gap-1.5 border-b border-[#1E212A] px-[26px] pt-[26px] pb-[20px]">
          <h2 className="text-[22px] font-bold">{t("onboarding.title")}</h2>
          <p className="text-[13px] text-[#8A8F9C]">{t("onboarding.subtitle")}</p>
        </div>

        <div className="flex flex-col gap-5 px-[26px] py-[22px]">
          <Field label={t("config.general.language")} hint={t("onboarding.languageHint")}>
            <Select
              value={settings?.app_language || "auto"}
              onValueChange={(v) => {
                save({ app_language: v });
                broadcastLanguage(v);
              }}
            >
              <SelectTrigger size="sm" style={trigger} className="rounded-lg px-3 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">{t("config.general.languageAuto")}</SelectItem>
                <SelectItem value="pt-BR">Português (Brasil)</SelectItem>
                <SelectItem value="en">English</SelectItem>
              </SelectContent>
            </Select>
          </Field>

          <Field label={t("onboarding.folder")} hint={t("onboarding.folderHint")}>
            <button
              type="button"
              onClick={pickFolder}
              className="flex items-center gap-2.5 rounded-lg border border-[#262A35] bg-[#1B1E27] px-3 py-2.5 text-left text-xs transition-colors hover:border-[#33374A]"
            >
              <FolderOpen className="size-4 shrink-0 text-primary" />
              <span className="min-w-0 flex-1 truncate" title={settings?.library_root}>
                {settings?.library_root || "—"}
              </span>
              <span className="shrink-0 font-semibold text-[#8A8F9C]">{t("onboarding.change")}</span>
            </button>
          </Field>

          <Field label={t("config.playback.player")} hint={t("onboarding.playerHint")}>
            <Select value={settings?.player_mode ?? "native"} onValueChange={(v) => save({ player_mode: v })}>
              <SelectTrigger size="sm" style={trigger} className="rounded-lg px-3 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="native">{t("config.playback.native")}</SelectItem>
                <SelectItem value="external">{t("config.playback.external")}</SelectItem>
              </SelectContent>
            </Select>
          </Field>
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-[#1E212A] px-[26px] py-[18px]">
          <button
            type="button"
            onClick={() => {
              setDismissed(true);
              finish();
            }}
            className="rounded-[10px] border border-[#33374A] px-[18px] py-2.5 text-[13px] font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
          >
            {t("onboarding.skip")}
          </button>
          <button
            type="button"
            onClick={() => {
              setDismissed(true);
              runTour();
            }}
            className="rounded-[10px] bg-primary px-5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90"
          >
            {t("onboarding.startTour")}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
