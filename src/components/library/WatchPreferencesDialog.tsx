import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { LanguageTagPicker } from "@/components/anime/LanguageTagPicker";
import { qualityOptions } from "@/lib/constants";
import { useTranslation } from "react-i18next";
import { setWatchPreferences, type Watch } from "@/lib/watches";
import { getAvailableLanguages } from "@/lib/nyaa";
import { getSettings } from "@/lib/tauri";
import { ffmpegInstall } from "@/lib/tools";
import { IrreversibleToggle } from "@/components/shared/IrreversibleToggle";

interface WatchPreferencesDialogProps {
  watch: Watch | null;
  onOpenChange: (open: boolean) => void;
}

export function WatchPreferencesDialog({ watch, onOpenChange }: WatchPreferencesDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [quality, setQuality] = useState("any");
  const [audioLangs, setAudioLangs] = useState<string[]>([]);
  const [subLangs, setSubLangs] = useState<string[]>([]);
  const [deleteAfterDays, setDeleteAfterDays] = useState("");
  const [notify, setNotify] = useState(true);
  const [stripAudio, setStripAudio] = useState(false);
  const [downscale, setDownscale] = useState(false);
  // Ligado na Config = vale pra todos; aqui só aparece travado.
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: getSettings, enabled: watch != null });
  const globalStrip = settings?.strip_unused_audio === "1";
  const globalDownscale = settings?.downscale_resolution === "720p";
  const totalEpisodes = watch?.episodes ?? 24;
  const [episodeRange, setEpisodeRange] = useState<[number, number]>([1, totalEpisodes]);
  const rangeIsFull = episodeRange[0] === 1 && episodeRange[1] === totalEpisodes;

  const { data: availableLangs } = useQuery({
    queryKey: ["nyaa-languages", watch?.query],
    queryFn: () => getAvailableLanguages(watch!.query),
    enabled: watch != null,
    staleTime: 10 * 60_000,
  });

  useEffect(() => {
    if (watch) {
      setQuality(watch.quality);
      setAudioLangs(watch.audio_lang ? watch.audio_lang.split(",") : []);
      setSubLangs(watch.sub_lang ? watch.sub_lang.split(",") : []);
      setDeleteAfterDays(watch.delete_after_days != null ? String(watch.delete_after_days) : "");
      setNotify(watch.notify_on_available);
      setStripAudio(watch.strip_audio);
      setDownscale(watch.max_resolution === "720p");
      const max = watch.episodes ?? 24;
      setEpisodeRange([watch.episode_start ?? 1, watch.episode_end ?? max]);
    }
  }, [watch]);

  const mutation = useMutation({
    mutationFn: () =>
      setWatchPreferences(watch!.id, {
        quality,
        audio_lang: audioLangs.length ? audioLangs.join(",") : null,
        sub_lang: subLangs.length ? subLangs.join(",") : null,
        // "0" não é retenção válida — apagaria no primeiro ciclo de limpeza
        // depois de ficar pronto. Trata igual a "nunca" em vez de aceitar.
        delete_after_days: deleteAfterDays.trim() && Number(deleteAfterDays) > 0 ? Number(deleteAfterDays) : null,
        notify_on_available: notify,
        episode_start: rangeIsFull ? null : episodeRange[0],
        episode_end: rangeIsFull ? null : episodeRange[1],
        max_resolution: downscale ? "720p" : null,
        strip_audio: stripAudio,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["watches"] });
      // Baixa o ffmpeg (se preciso) e já processa os episódios existentes.
      if (stripAudio || downscale) ffmpegInstall().catch(() => {});
      onOpenChange(false);
    },
  });

  return (
    <Dialog open={watch !== null} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "560px", width: "92vw" }}
        className="gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
      >
        {watch && (
          <>
            <div className="flex min-w-0 items-center justify-between gap-3 border-b border-[#1E212A] px-[22px] py-[18px]">
              <h2 className="min-w-0 truncate text-base font-bold">
                {t("detail.preferences")} · {watch.title}
              </h2>
              <button
                type="button"
                aria-label={t("common.close")}
                onClick={() => onOpenChange(false)}
                className="ml-3 flex size-[30px] shrink-0 items-center justify-center rounded-lg text-[#6C7180] transition-colors hover:bg-white/5 hover:text-foreground"
              >
                <X className="size-[15px]" strokeWidth={2} />
              </button>
            </div>

            <div className="flex flex-col gap-[18px] p-[22px]">
              <div className="flex items-center justify-between gap-5">
                <span className="text-[13px] font-semibold">{t("detail.quality")}</span>
                <Select value={quality} onValueChange={setQuality}>
                  <SelectTrigger
                    size="sm"
                    style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                    className="rounded-lg px-3 text-xs"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {qualityOptions().map((q) => (
                      <SelectItem key={q.value} value={q.value}>
                        {q.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="flex flex-col gap-2">
                <span className="text-[13px] font-semibold">{t("addAnime.audioLanguage")}</span>
                <LanguageTagPicker selected={audioLangs} onChange={setAudioLangs} available={availableLangs?.audio} />
              </div>
              <div className="flex flex-col gap-2">
                <span className="text-[13px] font-semibold">{t("detail.subtitle")}</span>
                <LanguageTagPicker selected={subLangs} onChange={setSubLangs} available={availableLangs?.subtitles} />
              </div>

              <div className="h-px bg-[#1E212A]" />

              <div className="flex items-center justify-between gap-5">
                <span className="text-[13px] font-semibold">{t("preferences.deleteAfterDays")}</span>
                <input
                  type="number"
                  min={1}
                  value={deleteAfterDays}
                  onChange={(e) => setDeleteAfterDays(e.target.value)}
                  placeholder={t("preferences.never")}
                  title={t("preferences.deleteAfterDaysHint")}
                  className="w-20 rounded-lg border border-[#262A35] bg-[#1B1E27] px-3 py-2 text-right text-xs text-foreground outline-none focus:border-primary"
                />
              </div>

              <div className="flex items-center justify-between gap-5">
                <span className="text-[13px] font-semibold">{t("preferences.notifyReady")}</span>
                <Switch checked={notify} onCheckedChange={setNotify} />
              </div>

              <div className="h-px bg-[#1E212A]" />
              <span className="text-[13px] font-semibold">{t("reduceSize.title")}</span>
              <IrreversibleToggle
                feature="stripAudio"
                scope="anime"
                checked={stripAudio}
                forcedOn={globalStrip}
                onCheckedChange={setStripAudio}
              />
              <IrreversibleToggle
                feature="downscale"
                scope="anime"
                checked={downscale}
                forcedOn={globalDownscale}
                onCheckedChange={setDownscale}
              />

              <div className="h-px bg-[#1E212A]" />

              <div className="flex flex-col gap-3">
                <div className="flex items-center justify-between">
                  <span className="text-[13px] font-semibold">{t("addAnime.whichEpisodes")}</span>
                  <span className="text-[12px] font-semibold text-primary">
                    {rangeIsFull
                      ? t("addAnime.allEpisodes")
                      : t("addAnime.episodeRange", { from: episodeRange[0], to: episodeRange[1] })}
                  </span>
                </div>
                <Slider
                  min={1}
                  max={totalEpisodes}
                  step={1}
                  value={episodeRange}
                  onValueChange={(v) => setEpisodeRange([v[0], v[1]])}
                  className="py-1.5"
                />
                <div className="flex items-center justify-between text-[11px] text-[#6C7180]">
                  <span>{t("common.episodeShort", { number: 1 })}</span>
                  <span>{t("common.episodeShort", { number: totalEpisodes })}</span>
                </div>
              </div>
            </div>

            <div className="flex items-center justify-end gap-3 border-t border-[#1E212A] px-[22px] py-[18px]">
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                className="rounded-[10px] border border-[#33374A] px-[18px] py-2.5 text-[13px] font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                disabled={mutation.isPending}
                onClick={() => mutation.mutate()}
                className="rounded-[10px] bg-primary px-5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {mutation.isPending ? t("common.saving") : t("common.save")}
              </button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
