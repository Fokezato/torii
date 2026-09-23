import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Bell, Download, Info, Plug, Play, Settings2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { broadcastLanguage, t } from "@/i18n";
import { getSettings, updateSettings } from "@/lib/tauri";
import { LanguageTagPicker } from "@/components/anime/LanguageTagPicker";
import { testJellyfinConnection } from "@/lib/jellyfin";
import { notify } from "@/lib/notify";
import { listWatches } from "@/lib/watches";
import { isAutostartEnabled, setAutostart } from "@/lib/autostart";
import { getVersion } from "@tauri-apps/api/app";
import { ffmpegInstall, ffmpegStatus } from "@/lib/tools";
import { IrreversibleToggle } from "@/components/shared/IrreversibleToggle";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

type Category = "geral" | "integracao" | "reproducao" | "downloads" | "notificacoes" | "sobre";

const CATEGORIES: { id: Category; icon: ComponentType<{ className?: string }> }[] = [
  { id: "geral", icon: Settings2 },
  { id: "integracao", icon: Plug },
  { id: "reproducao", icon: Play },
  { id: "downloads", icon: Download },
  { id: "notificacoes", icon: Bell },
  { id: "sobre", icon: Info },
];

function CategoryNav({ active, onChange }: { active: Category; onChange: (c: Category) => void }) {
  const { t } = useTranslation();
  return (
    <nav aria-label={t("config.categories")} className="flex w-[236px] shrink-0 flex-col gap-1">
      {CATEGORIES.map((cat) => {
        const Icon = cat.icon;
        const isActive = cat.id === active;
        return (
          <button
            key={cat.id}
            type="button"
            aria-current={isActive ? "page" : undefined}
            onClick={() => onChange(cat.id)}
            className={`flex items-center gap-3 rounded-[10px] px-3.5 py-2.5 text-left text-[13px] font-semibold transition-colors ${
              isActive ? "bg-secondary text-primary" : "text-[#9BA0AE] hover:text-foreground"
            }`}
          >
            <Icon className="size-[17px]" />
            {t(`config.cat.${cat.id}`)}
          </button>
        );
      })}
    </nav>
  );
}

function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="overflow-hidden rounded-[14px] border border-[#1E212A] bg-muted">
      <h3 className="border-b border-[#1E212A] px-[18px] py-[13px] text-[11px] font-bold tracking-wide text-[#6C7180] uppercase">
        {title}
      </h3>
      <div className="flex flex-col">{children}</div>
    </section>
  );
}

function SettingRow({
  label,
  description,
  badge,
  children,
}: {
  label: string;
  description?: string;
  badge?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-[#1E212A] px-[18px] py-[15px] last:border-b-0">
      <div className="flex flex-col gap-0.5">
        <span className="flex items-center gap-2 text-[13px] font-semibold">
          {label}
          {badge}
        </span>
        {description && <span className="text-xs text-[#6C7180]">{description}</span>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function InlineTextInput({
  value,
  onCommit,
  type = "text",
  placeholder,
}: {
  value: string;
  onCommit: (v: string) => void;
  type?: string;
  placeholder?: string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      type={type}
      value={draft}
      placeholder={placeholder}
      autoComplete="off"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) onCommit(draft);
      }}
      className="w-56 rounded-lg border border-[#262A35] bg-[#1B1E27] px-3 py-2 text-right text-xs text-foreground outline-none focus:border-primary"
    />
  );
}

function PathRow({
  label,
  description,
  value,
  onPick,
}: {
  label: string;
  description?: string;
  value: string;
  onPick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-6 border-b border-[#1E212A] px-[18px] py-[15px] last:border-b-0">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-[13px] font-semibold">{label}</span>
        <span className="truncate text-xs text-[#8A8F9C]">{value || description || t("config.notSet")}</span>
      </div>
      <button
        type="button"
        onClick={onPick}
        className="shrink-0 rounded-lg border border-[#33374A] px-3.5 py-2 text-xs font-semibold text-foreground transition-colors hover:bg-white/5"
      >
        {t("config.change")}
      </button>
    </div>
  );
}

type NotifyState = "idle" | "sent";

// Cada tipo liga numa chave de settings própria (notify_<id>), lida/gravada
// pelo motor no Rust (src-tauri/src/notify.rs) antes de emitir. "implemented"
// = false marca os 3 tipos que ainda não têm gatilho real no motor (o
// toggle já funciona e fica salvo, só falta a lógica que dispara sozinha —
// calendário de estreia, status assistido/não-assistido, e aviso antes da
// limpeza por retenção são features à parte, ainda não existem).
const SAMPLE_ANIME = "Mushoku Tensei: Jobless Reincarnation";

const NOTIFICATION_TYPES = [
  { id: "notify_found", key: "found", implemented: true, variant: "info" },
  { id: "notify_calendar", key: "calendar", implemented: false, variant: "info" },
  { id: "notify_error", key: "error", implemented: true, variant: "error" },
  { id: "notify_complete", key: "complete", implemented: true, variant: "success" },
  { id: "notify_jellyfin", key: "jellyfin", implemented: true, variant: "success" },
  { id: "notify_watch_reminder", key: "watchReminder", implemented: false, variant: "info" },
  { id: "notify_delete_reminder", key: "deleteReminder", implemented: false, variant: "error" },
] as const;

function NotificationTypeRow({
  type,
  enabled,
  masterEnabled,
  onToggle,
  sampleImage,
}: {
  type: (typeof NOTIFICATION_TYPES)[number];
  enabled: boolean;
  masterEnabled: boolean;
  onToggle: (checked: boolean) => void;
  sampleImage: string | null;
}) {
  const { t } = useTranslation();
  const [state, setState] = useState<NotifyState>("idle");
  const base = `config.notifications.${type.key}` as const;

  function handleTest() {
    notify(t(`${base}.title`), t(`${base}.body`, { anime: SAMPLE_ANIME }), type.variant, sampleImage);
    setState("sent");
    setTimeout(() => setState("idle"), 2000);
  }

  return (
    <SettingRow
      label={t(`${base}.label`)}
      description={t(`${base}.description`)}
      badge={
        !type.implemented && (
          <span className="rounded-full border border-[#33374A] px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-[#8A8F9C] uppercase">
            {t("config.comingSoon")}
          </span>
        )
      }
    >
      <div className="flex items-center gap-2.5">
        <button
          type="button"
          onClick={handleTest}
          className="shrink-0 rounded-lg border border-[#33374A] px-3 py-1.5 text-xs font-semibold text-foreground transition-colors hover:bg-white/5"
        >
          {state === "sent" ? t("config.sent") : t("config.test")}
        </button>
        <Switch checked={enabled && masterEnabled} disabled={!masterEnabled} onCheckedChange={onToggle} />
      </div>
    </SettingRow>
  );
}

// Rótulos montados na hora do render, pra seguir o idioma atual.
const minutes = (n: number) => t("config.duration.minutes", { count: n });
const hours = (n: number) => t("config.duration.hours", { count: n });
const days = (n: number) => t("config.duration.days", { count: n });

const pollOptions = () => [
  { value: "30", label: minutes(30) },
  { value: "60", label: hours(1) },
  { value: "300", label: hours(5) },
  { value: "1440", label: days(1) },
];

const CREDITS: { name: string; role: "tauri" | "vlc" | "ffmpeg" | "librqbit" | "anilist" | "aniskip" | "nyaa" | "ui"; license?: string }[] = [
  { name: "Tauri", role: "tauri", license: "MIT/Apache-2.0" },
  { name: "libVLC (VideoLAN)", role: "vlc", license: "LGPL-2.1" },
  { name: "FFmpeg", role: "ffmpeg", license: "LGPL" },
  { name: "librqbit", role: "librqbit", license: "Apache-2.0" },
  { name: "AniList", role: "anilist" },
  { name: "AniSkip", role: "aniskip" },
  { name: "Nyaa", role: "nyaa" },
  { name: "React, Tailwind CSS, shadcn/ui, Lucide", role: "ui", license: "MIT/ISC" },
];

// "0" = no próximo ciclo de limpeza (junto da checagem automática).
const watchedGraceOptions = () => [
  { value: "0", label: t("config.duration.rightAfter") },
  { value: "1", label: hours(1) },
  { value: "24", label: days(1) },
  { value: "72", label: days(3) },
  { value: "168", label: days(7) },
];
const retentionOptions = () => [
  { value: "never", label: t("config.duration.never") },
  ...[3, 7, 14, 30, 60].map((n) => ({ value: String(n), label: days(n) })),
];

export default function Config() {
  const { t } = useTranslation();
  const [active, setActive] = useState<Category>("geral");
  const queryClient = useQueryClient();
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [confirmJellyfin, setConfirmJellyfin] = useState(false);

  const testConnection = useMutation({ mutationFn: testJellyfinConnection });

  const { data, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
    refetchOnWindowFocus: false,
  });

  const { data: watches } = useQuery({ queryKey: ["watches"], queryFn: listWatches });
  const sampleCover = watches?.find((w) => w.cover_url)?.cover_url;

  const { data: appVersion } = useQuery({ queryKey: ["app-version"], queryFn: getVersion, staleTime: Infinity });
  const { data: autostartEnabled } = useQuery({
    queryKey: ["autostart"],
    queryFn: isAutostartEnabled,
  });
  const autostartMutation = useMutation({
    mutationFn: setAutostart,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["autostart"] }),
  });

  // Poll rápido só enquanto baixa, pra mostrar o progresso.
  const { data: ffmpeg } = useQuery({
    queryKey: ["ffmpeg-status"],
    queryFn: ffmpegStatus,
    refetchInterval: (query) => (query.state.data?.downloading ? 700 : 5000),
  });

  function installFfmpeg() {
    ffmpegInstall()
      .then(() => queryClient.invalidateQueries({ queryKey: ["ffmpeg-status"] }))
      .catch(() => {});
  }

  const patch = useMutation({
    mutationFn: (values: Record<string, string>) => updateSettings(values),
    onMutate: async (values) => {
      await queryClient.cancelQueries({ queryKey: ["settings"] });
      const previous = queryClient.getQueryData<Record<string, string>>(["settings"]);
      queryClient.setQueryData(["settings"], (old: Record<string, string> = {}) => ({
        ...old,
        ...values,
      }));
      return { previous };
    },
    onError: (_err, _values, context) => {
      if (context?.previous) queryClient.setQueryData(["settings"], context.previous);
    },
    onSuccess: () => setSavedAt(Date.now()),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  useEffect(() => {
    if (!savedAt) return;
    const timer = setTimeout(() => setSavedAt(null), 2000);
    return () => clearTimeout(timer);
  }, [savedAt]);

  const s = data ?? {};

  async function pickLibraryRoot() {
    const result = await open({ directory: true, defaultPath: s.library_root || undefined });
    if (typeof result === "string") patch.mutate({ library_root: result });
  }

  return (
    <div className="flex flex-col gap-[22px]">
      <h1 className="text-[26px] font-bold">{t("config.title")}</h1>

      <div className="flex items-start gap-8">
        <CategoryNav active={active} onChange={setActive} />

        <div className="flex min-w-0 flex-1 flex-col gap-5">
          <div className="flex flex-col gap-1">
            <h2 className="text-xl font-bold">{t(`config.cat.${active}`)}</h2>
            <p className="text-[13px] text-[#6C7180]">
              {t(`config.catDescription.${active}`)}
            </p>
          </div>

          {isLoading ? (
            <div className="space-y-4">
              <Skeleton className="h-32 rounded-[14px]" />
              <Skeleton className="h-24 rounded-[14px]" />
            </div>
          ) : (
            <>
              {active === "geral" && (
                <>
                  <SettingsGroup title={t("config.general.behavior")}>
                    <SettingRow
                      label={t("config.general.autostart")}
                      description={t("config.general.autostartHint")}
                    >
                      <Switch
                        checked={autostartEnabled ?? false}
                        disabled={autostartMutation.isPending}
                        onCheckedChange={(checked) => autostartMutation.mutate(checked)}
                      />
                    </SettingRow>
                    <SettingRow
                      label={t("config.general.closeAction")}
                      description={t("config.general.closeActionHint")}
                    >
                      <Select
                        value={s.close_action ?? "tray"}
                        onValueChange={(v) => patch.mutate({ close_action: v })}
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="tray">{t("config.general.closeTray")}</SelectItem>
                          <SelectItem value="quit">{t("config.general.closeQuit")}</SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow label={t("config.general.language")} description={t("config.general.languageHint")}>
                      <Select
                        value={s.app_language || "auto"}
                        onValueChange={(v) => {
                          patch.mutate({ app_language: v });
                          broadcastLanguage(v);
                        }}
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="auto">{t("config.general.languageAuto")}</SelectItem>
                          <SelectItem value="pt-BR">Português (Brasil)</SelectItem>
                          <SelectItem value="en">English</SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.general.appearance")}>
                    <SettingRow label={t("config.general.theme")} description={t("config.general.themeHint")}>
                      <span className="text-xs text-[#8A8F9C]">{t("config.general.themeDark")}</span>
                    </SettingRow>
                  </SettingsGroup>
                </>
              )}

              {active === "integracao" && (
                <SettingsGroup title="Jellyfin">
                  <SettingRow
                    label={t("config.jellyfin.enable")}
                    description={t("config.jellyfin.enableHint")}
                  >
                    <Switch
                      checked={s.jellyfin_mode === "1"}
                      onCheckedChange={(checked) => {
                        if (checked) setConfirmJellyfin(true);
                        else patch.mutate({ jellyfin_mode: "0" });
                      }}
                    />
                  </SettingRow>

                  {s.jellyfin_mode === "1" && (
                    <>
                      <SettingRow label={t("config.jellyfin.url")} description={t("config.jellyfin.urlHint")}>
                        <InlineTextInput
                          value={s.jellyfin_url ?? ""}
                          placeholder="http://localhost:8096"
                          onCommit={(v) => patch.mutate({ jellyfin_url: v })}
                        />
                      </SettingRow>
                      <SettingRow label={t("config.jellyfin.apiKey")} description={t("config.jellyfin.apiKeyHint")}>
                        <InlineTextInput
                          value={s.jellyfin_api_key ?? ""}
                          type="password"
                          onCommit={(v) => patch.mutate({ jellyfin_api_key: v })}
                        />
                      </SettingRow>
                      <PathRow
                        label={t("config.downloads.libraryFolder")}
                        description={t("config.jellyfin.libraryFolderHint")}
                        value={s.library_root ?? ""}
                        onPick={pickLibraryRoot}
                      />

                      <div className="flex items-center justify-between gap-6 px-[18px] py-[15px]">
                        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                          <span className="text-[13px] font-semibold">{t("config.jellyfin.testConnection")}</span>
                          {testConnection.isSuccess && (
                            <span className="text-xs text-[#6FC48A]">
                              {t("config.jellyfin.connected", { server: testConnection.data.server_name })}
                              {testConnection.data.version ? ` v${testConnection.data.version}` : ""}
                            </span>
                          )}
                          {testConnection.isError && (
                            <span className="text-xs text-destructive">{String(testConnection.error)}</span>
                          )}
                          {testConnection.isIdle && (
                            <span className="text-xs text-[#8A8F9C]">
                              {t("config.jellyfin.testHint")}
                            </span>
                          )}
                        </div>
                        <button
                          type="button"
                          disabled={testConnection.isPending}
                          onClick={() => testConnection.mutate()}
                          className="shrink-0 rounded-lg border border-[#33374A] px-3.5 py-2 text-xs font-semibold text-foreground transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                          {testConnection.isPending ? t("config.testing") : t("config.test")}
                        </button>
                      </div>
                    </>
                  )}
                </SettingsGroup>
              )}

              <AlertDialog open={confirmJellyfin} onOpenChange={setConfirmJellyfin}>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>{t("config.jellyfin.confirmTitle")}</AlertDialogTitle>
                    <AlertDialogDescription>
                      {t("config.jellyfin.confirmText")}
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
                    <AlertDialogAction onClick={() => patch.mutate({ jellyfin_mode: "1" })}>
                      {t("config.jellyfin.activate")}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>

              {active === "reproducao" && (
                <>
                  <SettingsGroup title={t("config.playback.player")}>
                    <SettingRow
                      label={t("config.playback.player")}
                      description={t("config.playback.playerHint")}
                    >
                      <Select
                        value={s.player_mode ?? "native"}
                        onValueChange={(v) => patch.mutate({ player_mode: v })}
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="native">{t("config.playback.native")}</SelectItem>
                          <SelectItem value="external">{t("config.playback.external")}</SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.playback.skipGroup")}>
                    {[
                      { key: "player_auto_skip_recap", label: t("config.playback.autoSkipRecap"), description: t("config.playback.autoSkipRecapHint") },
                      { key: "player_auto_skip_intro", label: t("config.playback.autoSkipIntro"), description: t("config.playback.autoSkipIntroHint") },
                      { key: "player_auto_skip_ending", label: t("config.playback.autoSkipEnding"), description: t("config.playback.autoSkipEndingHint") },
                      {
                        key: "player_next_after_ending",
                        label: t("config.playback.nextAfterEnding"),
                        description: t("config.playback.nextAfterEndingHint"),
                      },
                    ].map((row) => (
                      <SettingRow key={row.key} label={row.label} description={row.description}>
                        <Switch
                          checked={s[row.key] === "1"}
                          onCheckedChange={(checked) => patch.mutate({ [row.key]: checked ? "1" : "0" })}
                        />
                      </SettingRow>
                    ))}
                  </SettingsGroup>

                  <SettingsGroup title={t("config.playback.markGroup")}>
                    {[
                      { key: "player_mark_recap", label: t("player.segment.recap") },
                      { key: "player_mark_intro", label: t("player.segment.intro") },
                      { key: "player_mark_ending", label: t("player.segment.ending") },
                    ].map((row) => (
                      <SettingRow key={row.key} label={row.label} description={t("config.playback.markHint")}>
                        <Switch
                          checked={s[row.key] !== "0"}
                          onCheckedChange={(checked) => patch.mutate({ [row.key]: checked ? "1" : "0" })}
                        />
                      </SettingRow>
                    ))}
                    <p className="px-[18px] py-3 text-[11.5px] text-[#6C7180]">
                      {t("config.playback.aniskipNote")}
                    </p>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.playback.preferredGroup")}>
                    <div className="flex flex-col gap-2 px-[18px] py-3.5">
                      <span className="text-[13px] font-semibold">{t("config.playback.audio")}</span>
                      <span className="text-[11.5px] text-[#6C7180]">
                        {t("config.playback.audioHint")}
                      </span>
                      <LanguageTagPicker
                        selected={s.player_preferred_audio_langs ? s.player_preferred_audio_langs.split(",") : []}
                        onChange={(v) => patch.mutate({ player_preferred_audio_langs: v.join(",") })}
                      />
                    </div>
                    <div className="flex flex-col gap-2 px-[18px] py-3.5">
                      <span className="text-[13px] font-semibold">{t("config.playback.subtitle")}</span>
                      <LanguageTagPicker
                        selected={
                          s.player_preferred_subtitle_langs ? s.player_preferred_subtitle_langs.split(",") : []
                        }
                        onChange={(v) => patch.mutate({ player_preferred_subtitle_langs: v.join(",") })}
                      />
                    </div>
                  </SettingsGroup>
                </>
              )}

              {active === "downloads" && (
                <>
                  <SettingsGroup title={t("config.downloads.library")}>
                    <PathRow
                      label={t("config.downloads.libraryFolder")}
                      description={t("config.downloads.libraryFolderHint")}
                      value={s.library_root ?? ""}
                      onPick={pickLibraryRoot}
                    />
                  </SettingsGroup>

                  <SettingsGroup title={t("config.downloads.autoCheck")}>
                    <SettingRow label={t("config.downloads.checkEvery")} description={t("config.downloads.checkEveryHint")}>
                      <Select
                        value={s.poll_interval_minutes ?? "30"}
                        onValueChange={(v) => patch.mutate({ poll_interval_minutes: v })}
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {pollOptions().map((o) => (
                            <SelectItem key={o.value} value={o.value}>
                              {o.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow label={t("config.downloads.deleteAfter")} description={t("config.downloads.deleteAfterHint")}>
                      <Select
                        value={s.default_delete_after_days || "never"}
                        onValueChange={(v) =>
                          patch.mutate({ default_delete_after_days: v === "never" ? "" : v })
                        }
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {retentionOptions().map((o) => (
                            <SelectItem key={o.value} value={o.value}>
                              {o.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow
                      label={t("config.downloads.pause")}
                      description={t("config.downloads.pauseHint")}
                    >
                      <Switch
                        checked={s.paused === "1"}
                        onCheckedChange={(checked) => patch.mutate({ paused: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.downloads.cleanup")}>
                    <SettingRow
                      label={t("config.downloads.deleteWatched")}
                      description={t("config.downloads.deleteWatchedHint")}
                    >
                      <Switch
                        checked={s.delete_after_watched === "1"}
                        onCheckedChange={(checked) => patch.mutate({ delete_after_watched: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                    {s.delete_after_watched === "1" && (
                      <SettingRow label={t("config.downloads.waitBeforeDelete")} description={t("config.downloads.waitBeforeDeleteHint")}>
                        <Select
                          value={s.delete_after_watched_hours ?? "24"}
                          onValueChange={(v) => patch.mutate({ delete_after_watched_hours: v })}
                        >
                          <SelectTrigger
                            size="sm"
                            style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                            className="rounded-lg px-3 text-xs"
                          >
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {watchedGraceOptions().map((o) => (
                              <SelectItem key={o.value} value={o.value}>
                                {o.label}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </SettingRow>
                    )}
                  </SettingsGroup>

                  <SettingsGroup title={t("reduceSize.title")}>
                    <p className="border-b border-[#1E212A] px-[18px] py-3 text-[11.5px] text-[#6C7180]">
                      {t("config.downloads.reduceIntro")}
                    </p>
                    <div className="border-b border-[#1E212A] px-[18px] py-[15px]">
                      <IrreversibleToggle
                        feature="stripAudio"
                        scope="global"
                        checked={s.strip_unused_audio === "1"}
                        onCheckedChange={(checked) => {
                          patch.mutate({ strip_unused_audio: checked ? "1" : "0" });
                          if (checked) installFfmpeg();
                        }}
                      />
                    </div>
                    <div className="border-b border-[#1E212A] px-[18px] py-[15px]">
                      <IrreversibleToggle
                        feature="downscale"
                        scope="global"
                        checked={s.downscale_resolution === "720p"}
                        onCheckedChange={(checked) => {
                          patch.mutate({ downscale_resolution: checked ? "720p" : "original" });
                          if (checked) installFfmpeg();
                        }}
                      />
                    </div>
                    {(s.strip_unused_audio === "1" || s.downscale_resolution === "720p") && (
                      <div className="flex flex-col gap-1 px-[18px] py-3 text-xs">
                        {s.strip_unused_audio === "1" && !s.player_preferred_audio_langs && (
                          <span className="text-[#E5A34B]">
                            {t("config.downloads.noAudioLangs")}
                          </span>
                        )}
                        {ffmpeg?.downloading ? (
                          <span className="text-[#8A8F9C]">
                            {t("config.downloads.ffmpegDownloading", { percent: Math.round(ffmpeg.progress) })}
                          </span>
                        ) : ffmpeg?.error ? (
                          <span className="flex items-center gap-2 text-destructive">
                            {ffmpeg.error}
                            <button
                              type="button"
                              onClick={installFfmpeg}
                              className="rounded-md border border-[#33374A] px-2 py-0.5 text-[11px] font-semibold text-foreground hover:bg-white/5"
                            >
                              {t("config.downloads.retry")}
                            </button>
                          </span>
                        ) : ffmpeg?.installed ? (
                          <span className="text-[#6FC48A]">
                            {t("config.downloads.ffmpegReady")}
                          </span>
                        ) : null}
                      </div>
                    )}
                  </SettingsGroup>
                </>
              )}

              {active === "notificacoes" && (
                <>
                  <SettingsGroup title={t("config.notifications.general")}>
                    <SettingRow
                      label={t("config.notifications.master")}
                      description={t("config.notifications.masterHint")}
                    >
                      <Switch
                        checked={s.notify_master !== "0"}
                        onCheckedChange={(checked) => patch.mutate({ notify_master: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                    <SettingRow label={t("config.notifications.sound")} description={t("config.notifications.soundHint")}>
                      <Switch
                        checked={s.notify_sound !== "0"}
                        disabled={s.notify_master === "0"}
                        onCheckedChange={(checked) => patch.mutate({ notify_sound: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.notifications.types")}>
                    {NOTIFICATION_TYPES.map((type) => (
                      <NotificationTypeRow
                        key={type.id}
                        type={type}
                        enabled={s[type.id] !== "0"}
                        masterEnabled={s.notify_master !== "0"}
                        onToggle={(checked) => patch.mutate({ [type.id]: checked ? "1" : "0" })}
                        sampleImage={sampleCover ?? null}
                      />
                    ))}
                  </SettingsGroup>
                </>
              )}

              {active === "sobre" && (
                <>
                  <SettingsGroup title="Torii">
                    <SettingRow label={t("config.about.version")}>
                      <span className="text-xs text-[#8A8F9C]">{appVersion ?? "…"}</span>
                    </SettingRow>
                    <SettingRow label="Stack">
                      <span className="text-xs text-[#8A8F9C]">Tauri v2 · Rust · React</span>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.about.credits")}>
                    <div className="flex flex-col gap-3 px-[18px] py-4 text-xs leading-relaxed text-[#8A8F9C]">
                      <p>{t("config.about.creditsIntro")}</p>
                      <ul className="flex flex-col gap-1.5">
                        {CREDITS.map((c) => (
                          <li key={c.name}>
                            <span className="font-semibold text-[#C7CAD3]">{c.name}</span>
                            {" · "}
                            {t(`config.about.roles.${c.role}`)}
                            {c.license && <span className="text-[#6C7180]"> ({c.license})</span>}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </SettingsGroup>

                  <SettingsGroup title={t("config.about.legal")}>
                    <div className="px-[18px] py-4 text-xs leading-relaxed text-[#8A8F9C]">
                      {t("config.about.legalText")}
                    </div>
                  </SettingsGroup>
                </>
              )}
            </>
          )}

          <p className="text-[11px] text-[#4E5361]">
            {t("config.autoSaved")}
            {patch.isPending && ` ${t("config.saving")}`}
            {savedAt && !patch.isPending && ` ${t("config.saved")}`}
          </p>
        </div>
      </div>
    </div>
  );
}
