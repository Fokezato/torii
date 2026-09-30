import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw, Settings } from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { getSettings, updateSettings } from "@/lib/tauri";
import {
  DEFAULT_AMBIENT,
  ambientFromSettings,
  ambientToSettings,
  broadcastAmbient,
  type AmbientSettings,
} from "@/lib/ambient";
import {
  DEFAULT_SUBTITLE_STYLE,
  SUBTITLE_COLORS,
  subtitleStyleToSettings,
  type SubtitleFont,
  type SubtitleStyle,
} from "@/lib/subtitleStyle";

function useDebouncedSave(delay = 500) {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  return (values: Record<string, string>) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => updateSettings(values).catch(() => {}), delay);
  };
}

export function PlayerSettingsButton({
  open,
  setOpen,
  subStyle,
  onSubStyleChange,
}: {
  open: boolean;
  setOpen: (open: boolean) => void;
  subStyle: SubtitleStyle;
  onSubStyleChange: (next: SubtitleStyle) => void;
}) {
  const { t } = useTranslation();
  const [ambient, setAmbient] = useState<AmbientSettings>(DEFAULT_AMBIENT);
  const loadedRef = useRef(false);
  const saveAmbient = useDebouncedSave();
  const saveSubtitle = useDebouncedSave();

  useEffect(() => {
    getSettings()
      .then((s) => {
        setAmbient(ambientFromSettings(s));
        loadedRef.current = true;
      })
      .catch(() => {});
  }, []);

  function patchAmbient(p: Partial<AmbientSettings>) {
    setAmbient((prev) => {
      const next = { ...prev, ...p };
      broadcastAmbient(next);
      if (loadedRef.current) saveAmbient(ambientToSettings(next));
      return next;
    });
  }

  function patchSubtitle(p: Partial<SubtitleStyle>) {
    const next = { ...subStyle, ...p };
    onSubStyleChange(next);
    saveSubtitle(subtitleStyleToSettings(next));
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t("player.settings.title")}
          onClick={(e) => e.stopPropagation()}
          className="flex size-8 items-center justify-center rounded-full text-white transition-colors hover:bg-white/10"
        >
          <Settings className="size-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="top"
        onClick={(e) => e.stopPropagation()}
        className="max-h-[75vh] w-80 overflow-y-auto border-[#262A35] bg-[#15171D] p-3 text-foreground"
      >
        <Section
          title={t("player.settings.ambient")}
          onReset={() => patchAmbient(DEFAULT_AMBIENT)}
          resetLabel={t("player.settings.reset")}
        >
          <Row label={t("player.settings.ambientOn")}>
            <Switch checked={ambient.enabled} onCheckedChange={(v) => patchAmbient({ enabled: v })} />
          </Row>
          {ambient.enabled && (
            <>
              <SliderRow
                label={t("player.settings.intensity")}
                value={ambient.intensity}
                min={10}
                max={100}
                format={(v) => `${v}%`}
                onChange={(v) => patchAmbient({ intensity: v })}
              />
              <SliderRow
                label={t("player.settings.glowSize")}
                value={ambient.size}
                min={110}
                max={180}
                format={(v) => `${v}%`}
                onChange={(v) => patchAmbient({ size: v })}
              />
              <SliderRow
                label={t("player.settings.smoothness")}
                value={ambient.smoothness}
                min={0}
                max={100}
                format={(v) => `${v}%`}
                onChange={(v) => patchAmbient({ smoothness: v })}
              />
            </>
          )}
        </Section>

        <div className="my-3 h-px bg-[#1E212A]" />

        <Section
          title={t("player.settings.subtitles")}
          onReset={() => patchSubtitle({ ...DEFAULT_SUBTITLE_STYLE, mode: subStyle.mode })}
          resetLabel={t("player.settings.reset")}
        >
          <Segmented
            value={subStyle.mode}
            options={[
              { value: "original", label: t("player.settings.subOriginal") },
              { value: "custom", label: t("player.settings.subCustom") },
            ]}
            onChange={(v) => patchSubtitle({ mode: v as SubtitleStyle["mode"] })}
          />
          <p className="text-[11px] leading-snug text-[#6C7180]">
            {subStyle.mode === "custom" ? t("player.settings.subCustomHint") : t("player.settings.subOriginalHint")}
          </p>
          {subStyle.mode === "custom" && (
            <>
              <SliderRow
                label={t("player.settings.subSize")}
                value={subStyle.size}
                min={50}
                max={250}
                step={10}
                format={(v) => `${v}%`}
                onChange={(v) => patchSubtitle({ size: v })}
              />
              <div className="flex flex-col gap-1.5">
                <span className="text-xs">{t("player.settings.subFont")}</span>
                <Segmented
                  value={subStyle.font}
                  options={[
                    { value: "sans", label: t("player.settings.fontSans") },
                    { value: "serif", label: t("player.settings.fontSerif") },
                    { value: "rounded", label: t("player.settings.fontRounded") },
                  ]}
                  onChange={(v) => patchSubtitle({ font: v as SubtitleFont })}
                />
              </div>
              <Row label={t("player.settings.subColor")}>
                <div className="flex gap-1.5">
                  {SUBTITLE_COLORS.map((c) => (
                    <button
                      key={c}
                      type="button"
                      aria-label={c}
                      aria-pressed={subStyle.color === c}
                      onClick={() => patchSubtitle({ color: c })}
                      className={`size-5 rounded-full border-2 ${subStyle.color === c ? "border-primary" : "border-transparent"}`}
                      style={{ background: c }}
                    />
                  ))}
                </div>
              </Row>
              <div className="flex flex-col gap-1.5">
                <span className="text-xs">{t("player.settings.subOutline")}</span>
                <Segmented
                  value={String(subStyle.outline)}
                  options={[
                    { value: "0", label: t("player.settings.outlineNone") },
                    { value: "1", label: t("player.settings.outlineThin") },
                    { value: "2", label: t("player.settings.outlineNormal") },
                    { value: "3", label: t("player.settings.outlineThick") },
                  ]}
                  onChange={(v) => patchSubtitle({ outline: Number(v) })}
                />
              </div>
              <SliderRow
                label={t("player.settings.subBackground")}
                value={subStyle.background}
                min={0}
                max={100}
                step={5}
                format={(v) => `${v}%`}
                onChange={(v) => patchSubtitle({ background: v })}
              />
              <SliderRow
                label={t("player.settings.subPosition")}
                value={subStyle.position}
                min={0}
                max={25}
                format={(v) => `${v}%`}
                onChange={(v) => patchSubtitle({ position: v })}
              />
            </>
          )}
        </Section>
      </PopoverContent>
    </Popover>
  );
}

function Section({
  title,
  onReset,
  resetLabel,
  children,
}: {
  title: string;
  onReset: () => void;
  resetLabel: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[10px] font-bold tracking-wide text-[#6C7180] uppercase">{title}</span>
        <button
          type="button"
          onClick={onReset}
          className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-semibold text-[#9BA0AE] transition-colors hover:bg-white/5 hover:text-foreground"
        >
          <RotateCcw className="size-3" />
          {resetLabel}
        </button>
      </div>
      {children}
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-xs">{label}</span>
      {children}
    </div>
  );
}

function SliderRow({
  label,
  value,
  min,
  max,
  step = 1,
  format,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between text-xs">
        <span>{label}</span>
        <span className="text-[#6C7180]">{format(value)}</span>
      </div>
      <Slider value={[value]} min={min} max={max} step={step} onValueChange={([v]) => onChange(v)} />
    </div>
  );
}

function Segmented({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex gap-1 rounded-lg border border-[#262A35] bg-[#1B1E27] p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          aria-pressed={value === o.value}
          onClick={() => onChange(o.value)}
          className={`flex-1 rounded-md px-2 py-1 text-[11px] font-semibold transition-colors ${
            value === o.value ? "bg-primary text-primary-foreground" : "text-[#9BA0AE] hover:text-foreground"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
