import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Bell, Download, Info, Plug, Play, Settings2 } from "lucide-react";
import { getSettings, updateSettings } from "@/lib/tauri";
import { LanguageTagPicker } from "@/components/anime/LanguageTagPicker";
import { testJellyfinConnection } from "@/lib/jellyfin";
import { notify } from "@/lib/notify";
import { listWatches } from "@/lib/watches";
import { isAutostartEnabled, setAutostart } from "@/lib/autostart";
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

const CATEGORIES: { id: Category; label: string; icon: ComponentType<{ className?: string }> }[] = [
  { id: "geral", label: "Geral", icon: Settings2 },
  { id: "integracao", label: "Integração", icon: Plug },
  { id: "reproducao", label: "Reprodução", icon: Play },
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "notificacoes", label: "Notificações", icon: Bell },
  { id: "sobre", label: "Sobre", icon: Info },
];

function CategoryNav({ active, onChange }: { active: Category; onChange: (c: Category) => void }) {
  return (
    <nav aria-label="Categorias de configurações" className="flex w-[236px] shrink-0 flex-col gap-1">
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
            {cat.label}
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
  return (
    <div className="flex items-center justify-between gap-6 border-b border-[#1E212A] px-[18px] py-[15px] last:border-b-0">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-[13px] font-semibold">{label}</span>
        <span className="truncate text-xs text-[#8A8F9C]">{value || description || "não definido"}</span>
      </div>
      <button
        type="button"
        onClick={onPick}
        className="shrink-0 rounded-lg border border-[#33374A] px-3.5 py-2 text-xs font-semibold text-foreground transition-colors hover:bg-white/5"
      >
        Alterar
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
const NOTIFICATION_TYPES = [
  {
    id: "notify_found",
    label: "Episódio encontrado",
    description: "Quando o motor acha um torrent compatível e começa a baixar",
    implemented: true,
    sample: {
      title: "Novo episódio encontrado",
      body: "Mushoku Tensei: Jobless Reincarnation S03E11 — iniciando download",
      variant: "info",
    },
  },
  {
    id: "notify_calendar",
    label: "Estreia do calendário",
    description: "No dia em que um episódio novo estreia, segundo o calendário",
    implemented: false,
    sample: {
      title: "Estreia hoje",
      body: "Mushoku Tensei: Jobless Reincarnation S03E12 estreia hoje",
      variant: "info",
    },
  },
  {
    id: "notify_error",
    label: "Problema no download",
    description: "Quando um download falha",
    implemented: true,
    sample: {
      title: "Falha no download",
      body: "Não foi possível baixar Mushoku Tensei: Jobless Reincarnation S03E11",
      variant: "error",
    },
  },
  {
    id: "notify_complete",
    label: "Download concluído",
    description: "Quando o episódio termina de baixar",
    implemented: true,
    sample: {
      title: "Download concluído",
      body: "Mushoku Tensei: Jobless Reincarnation S03E11 já está pronto pra assistir",
      variant: "success",
    },
  },
  {
    id: "notify_jellyfin",
    label: "Disponível no Jellyfin",
    description: "Quando o episódio aparece na sua biblioteca Jellyfin",
    implemented: true,
    sample: {
      title: "Disponível no Jellyfin",
      body: "Mushoku Tensei: Jobless Reincarnation S03E11",
      variant: "success",
    },
  },
  {
    id: "notify_watch_reminder",
    label: "Lembrete pra assistir",
    description: "Episódio baixado há um tempo e ainda não foi assistido",
    implemented: false,
    sample: {
      title: "Já baixou, falta assistir",
      body: "Mushoku Tensei: Jobless Reincarnation S03E11 tá esperando você",
      variant: "info",
    },
  },
  {
    id: "notify_delete_reminder",
    label: "Lembrete de exclusão",
    description: "Avisa antes de um episódio ser apagado pela retenção",
    implemented: false,
    sample: {
      title: "Vai ser apagado em breve",
      body: "Mushoku Tensei: Jobless Reincarnation S03E11 será removido em 2 dias",
      variant: "error",
    },
  },
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
  const [state, setState] = useState<NotifyState>("idle");

  function handleTest() {
    notify(type.sample.title, type.sample.body, type.sample.variant, sampleImage);
    setState("sent");
    setTimeout(() => setState("idle"), 2000);
  }

  return (
    <SettingRow
      label={type.label}
      description={type.description}
      badge={
        !type.implemented && (
          <span className="rounded-full border border-[#33374A] px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-[#8A8F9C] uppercase">
            Em breve
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
          {state === "sent" ? "Enviada" : "Testar"}
        </button>
        <Switch checked={enabled && masterEnabled} disabled={!masterEnabled} onCheckedChange={onToggle} />
      </div>
    </SettingRow>
  );
}

const POLL_OPTIONS = [
  { value: "30", label: "30 minutos" },
  { value: "60", label: "1 hora" },
  { value: "300", label: "5 horas" },
  { value: "1440", label: "1 dia" },
];
const CREDITS: { name: string; role: string; license?: string }[] = [
  { name: "Tauri", role: "base do app", license: "MIT/Apache-2.0" },
  { name: "libVLC (VideoLAN)", role: "reprodução de vídeo", license: "LGPL-2.1" },
  { name: "FFmpeg", role: "remover áudios extras e reduzir resolução, baixado sob demanda", license: "LGPL" },
  { name: "librqbit", role: "downloads por torrent", license: "Apache-2.0" },
  { name: "AniList", role: "catálogo, capas e informações dos animes" },
  { name: "AniSkip", role: "trechos de abertura, encerramento e resumo" },
  { name: "Nyaa", role: "busca de episódios" },
  { name: "React, Tailwind CSS, shadcn/ui e Lucide", role: "interface", license: "MIT/ISC" },
];

// "0" = no próximo ciclo de limpeza (junto da checagem automática).
const WATCHED_GRACE_OPTIONS = [
  { value: "0", label: "Logo depois" },
  { value: "1", label: "1 hora" },
  { value: "24", label: "1 dia" },
  { value: "72", label: "3 dias" },
  { value: "168", label: "7 dias" },
];
const RETENTION_OPTIONS = [
  { value: "never", label: "Nunca" },
  { value: "3", label: "3 dias" },
  { value: "7", label: "7 dias" },
  { value: "14", label: "14 dias" },
  { value: "30", label: "30 dias" },
  { value: "60", label: "60 dias" },
];

export default function Config() {
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
    const t = setTimeout(() => setSavedAt(null), 2000);
    return () => clearTimeout(t);
  }, [savedAt]);

  const s = data ?? {};

  async function pickLibraryRoot() {
    const result = await open({ directory: true, defaultPath: s.library_root || undefined });
    if (typeof result === "string") patch.mutate({ library_root: result });
  }

  return (
    <div className="flex flex-col gap-[22px]">
      <h1 className="text-[26px] font-bold">Configurações</h1>

      <div className="flex items-start gap-8">
        <CategoryNav active={active} onChange={setActive} />

        <div className="flex min-w-0 flex-1 flex-col gap-5">
          <div className="flex flex-col gap-1">
            <h2 className="text-xl font-bold">{CATEGORIES.find((c) => c.id === active)?.label}</h2>
            <p className="text-[13px] text-[#6C7180]">
              {active === "geral" && "Comportamento básico do app, idioma e aparência."}
              {active === "integracao" && "Conexão com serviços externos."}
              {active === "reproducao" && "Player usado pra abrir os episódios."}
              {active === "downloads" && "Pasta da biblioteca e comportamento da checagem automática."}
              {active === "notificacoes" && "Avisos de novo episódio, download e mais."}
              {active === "sobre" && "Versão e informações do Torii."}
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
                  <SettingsGroup title="Comportamento">
                    <SettingRow
                      label="Abrir com o Windows"
                      description="Inicia o Torii minimizado na bandeja junto com o Windows"
                    >
                      <Switch
                        checked={autostartEnabled ?? false}
                        disabled={autostartMutation.isPending}
                        onCheckedChange={(checked) => autostartMutation.mutate(checked)}
                      />
                    </SettingRow>
                    <SettingRow
                      label="Ao clicar em fechar"
                      description="Minimizar pra bandeja deixa o Torii rodando em segundo plano, buscando e baixando episódios novos. Fechar encerra o app de vez."
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
                          <SelectItem value="tray">Minimizar pra bandeja</SelectItem>
                          <SelectItem value="quit">Fechar o app</SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow label="Idioma" description="Idioma da interface">
                      <Select
                        value={s.app_language ?? "pt-BR"}
                        onValueChange={(v) => patch.mutate({ app_language: v })}
                      >
                        <SelectTrigger
                          size="sm"
                          style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
                          className="rounded-lg px-3 text-xs"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="pt-BR">Português (Brasil)</SelectItem>
                          <SelectItem value="en" disabled>
                            English (em breve)
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title="Aparência">
                    <SettingRow label="Tema" description="Torii usa tema escuro fixo por enquanto">
                      <span className="text-xs text-[#8A8F9C]">Escuro</span>
                    </SettingRow>
                  </SettingsGroup>
                </>
              )}

              {active === "integracao" && (
                <SettingsGroup title="Jellyfin">
                  <SettingRow
                    label="Ativar integração Jellyfin"
                    description="Torii vira um servidor automático de busca e download pro Jellyfin"
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
                      <SettingRow label="URL do servidor" description="Endereço do seu Jellyfin">
                        <InlineTextInput
                          value={s.jellyfin_url ?? ""}
                          placeholder="http://localhost:8096"
                          onCommit={(v) => patch.mutate({ jellyfin_url: v })}
                        />
                      </SettingRow>
                      <SettingRow label="API key" description="Gerada no painel admin do Jellyfin">
                        <InlineTextInput
                          value={s.jellyfin_api_key ?? ""}
                          type="password"
                          onCommit={(v) => patch.mutate({ jellyfin_api_key: v })}
                        />
                      </SettingRow>
                      <PathRow
                        label="Pasta da biblioteca"
                        description="Pasta que o Jellyfin monitora — os episódios baixados vão direto pra lá"
                        value={s.library_root ?? ""}
                        onPick={pickLibraryRoot}
                      />

                      <div className="flex items-center justify-between gap-6 px-[18px] py-[15px]">
                        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                          <span className="text-[13px] font-semibold">Testar conexão</span>
                          {testConnection.isSuccess && (
                            <span className="text-xs text-[#6FC48A]">
                              Conectado — {testConnection.data.server_name}
                              {testConnection.data.version ? ` v${testConnection.data.version}` : ""}
                            </span>
                          )}
                          {testConnection.isError && (
                            <span className="text-xs text-destructive">{String(testConnection.error)}</span>
                          )}
                          {testConnection.isIdle && (
                            <span className="text-xs text-[#8A8F9C]">
                              Confirma se a URL e a API key estão certas
                            </span>
                          )}
                        </div>
                        <button
                          type="button"
                          disabled={testConnection.isPending}
                          onClick={() => testConnection.mutate()}
                          className="shrink-0 rounded-lg border border-[#33374A] px-3.5 py-2 text-xs font-semibold text-foreground transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                          {testConnection.isPending ? "Testando..." : "Testar"}
                        </button>
                      </div>
                    </>
                  )}
                </SettingsGroup>
              )}

              <AlertDialog open={confirmJellyfin} onOpenChange={setConfirmJellyfin}>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Ativar modo Jellyfin?</AlertDialogTitle>
                    <AlertDialogDescription>
                      No modo Jellyfin, a organização e a reprodução passam a ser gerenciadas pelo
                      Jellyfin. O Torii deixa de atuar como hub e passa a funcionar só como um sistema
                      automático de busca e download, entregando os episódios direto na pasta da sua
                      biblioteca Jellyfin.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>Cancelar</AlertDialogCancel>
                    <AlertDialogAction onClick={() => patch.mutate({ jellyfin_mode: "1" })}>
                      Ativar
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>

              {active === "reproducao" && (
                <>
                  <SettingsGroup title="Player">
                    <SettingRow
                      label="Player"
                      description="O nativo toca dentro do Torii, com pular abertura, idioma preferido e continuar de onde parou. O externo abre o episódio no player padrão do Windows."
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
                          <SelectItem value="native">Nativo</SelectItem>
                          <SelectItem value="external">Externo</SelectItem>
                        </SelectContent>
                      </Select>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title="Pular trechos">
                    {[
                      { key: "player_auto_skip_recap", label: "Pular resumo automaticamente", description: "Resumo do episódio anterior, no começo" },
                      { key: "player_auto_skip_intro", label: "Pular abertura automaticamente", description: "Avança sozinho ao entrar na abertura" },
                      { key: "player_auto_skip_ending", label: "Pular encerramento automaticamente", description: "Avança sozinho ao entrar no encerramento" },
                      {
                        key: "player_next_after_ending",
                        label: "Ir pro próximo episódio depois do encerramento",
                        description: "Pula a prévia/créditos que vêm depois do encerramento e abre o próximo episódio baixado",
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

                  <SettingsGroup title="Marcar na barra do tempo">
                    {[
                      { key: "player_mark_recap", label: "Resumo" },
                      { key: "player_mark_intro", label: "Abertura" },
                      { key: "player_mark_ending", label: "Encerramento" },
                    ].map((row) => (
                      <SettingRow key={row.key} label={row.label} description="Destaca o trecho em amarelo na barra do player">
                        <Switch
                          checked={s[row.key] !== "0"}
                          onCheckedChange={(checked) => patch.mutate({ [row.key]: checked ? "1" : "0" })}
                        />
                      </SettingRow>
                    ))}
                    <p className="px-[18px] py-3 text-[11.5px] text-[#6C7180]">
                      Os trechos vêm do AniSkip, uma base colaborativa. Nem todo episódio tem.
                    </p>
                  </SettingsGroup>

                  <SettingsGroup title="Idioma preferido do player">
                    <div className="flex flex-col gap-2 px-[18px] py-3.5">
                      <span className="text-[13px] font-semibold">Áudio</span>
                      <span className="text-[11.5px] text-[#6C7180]">
                        Os episódios costumam vir com várias faixas de áudio e legenda. O player mostra só os idiomas
                        escolhidos aqui (o japonês original sempre aparece) e já abre o episódio no primeiro disponível.
                      </span>
                      <LanguageTagPicker
                        selected={s.player_preferred_audio_langs ? s.player_preferred_audio_langs.split(",") : []}
                        onChange={(v) => patch.mutate({ player_preferred_audio_langs: v.join(",") })}
                      />
                    </div>
                    <div className="flex flex-col gap-2 px-[18px] py-3.5">
                      <span className="text-[13px] font-semibold">Legenda</span>
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
                  <SettingsGroup title="Biblioteca">
                    <PathRow
                      label="Pasta da biblioteca"
                      description="Base onde as pastas por anime são criadas"
                      value={s.library_root ?? ""}
                      onPick={pickLibraryRoot}
                    />
                  </SettingsGroup>

                  <SettingsGroup title="Checagem automática">
                    <SettingRow label="Checar a cada" description="Intervalo entre buscas por novos episódios">
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
                          {POLL_OPTIONS.map((o) => (
                            <SelectItem key={o.value} value={o.value}>
                              {o.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow label="Apagar após" description="Padrão global de retenção">
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
                          {RETENTION_OPTIONS.map((o) => (
                            <SelectItem key={o.value} value={o.value}>
                              {o.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </SettingRow>
                    <SettingRow
                      label="Pausar checagem automática"
                      description="Nenhum novo episódio será buscado"
                    >
                      <Switch
                        checked={s.paused === "1"}
                        onCheckedChange={(checked) => patch.mutate({ paused: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title="Limpeza">
                    <SettingRow
                      label="Apagar depois de assistir"
                      description="Remove o arquivo sozinho depois que você assiste até o encerramento. O episódio não é baixado de novo."
                    >
                      <Switch
                        checked={s.delete_after_watched === "1"}
                        onCheckedChange={(checked) => patch.mutate({ delete_after_watched: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                    {s.delete_after_watched === "1" && (
                      <SettingRow label="Esperar antes de apagar" description="Tempo pra rever antes do arquivo sumir">
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
                            {WATCHED_GRACE_OPTIONS.map((o) => (
                              <SelectItem key={o.value} value={o.value}>
                                {o.label}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </SettingRow>
                    )}
                  </SettingsGroup>

                  <SettingsGroup title="Reduzir tamanho dos arquivos">
                    <p className="border-b border-[#1E212A] px-[18px] py-3 text-[11.5px] text-[#6C7180]">
                      Ligado aqui, vale pra todos os animes. Desligado, você escolhe por anime: ao adicionar (em
                      Avançado) ou nas preferências do anime na Biblioteca.
                    </p>
                    <div className="border-b border-[#1E212A] px-[18px] py-[15px]">
                      <IrreversibleToggle
                        feature="strip_audio"
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
                            Configure os idiomas de áudio preferidos em Reprodução — sem eles nenhum áudio é removido.
                          </span>
                        )}
                        {ffmpeg?.downloading ? (
                          <span className="text-[#8A8F9C]">
                            Baixando ferramenta (ffmpeg, ~80MB)… {Math.round(ffmpeg.progress)}%
                          </span>
                        ) : ffmpeg?.error ? (
                          <span className="flex items-center gap-2 text-destructive">
                            {ffmpeg.error}
                            <button
                              type="button"
                              onClick={installFfmpeg}
                              className="rounded-md border border-[#33374A] px-2 py-0.5 text-[11px] font-semibold text-foreground hover:bg-white/5"
                            >
                              Tentar de novo
                            </button>
                          </span>
                        ) : ffmpeg?.installed ? (
                          <span className="text-[#6FC48A]">
                            Ferramenta pronta. Episódios já baixados também são processados, um por vez, em segundo plano. Isso
                            não tem volta — o original é substituído.
                          </span>
                        ) : null}
                      </div>
                    )}
                  </SettingsGroup>
                </>
              )}

              {active === "notificacoes" && (
                <>
                  <SettingsGroup title="Geral">
                    <SettingRow
                      label="Ativar notificações"
                      description="Chave mestra — desliga todos os tipos de uma vez"
                    >
                      <Switch
                        checked={s.notify_master !== "0"}
                        onCheckedChange={(checked) => patch.mutate({ notify_master: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                    <SettingRow label="Som" description="Toca um som quando uma notificação aparece">
                      <Switch
                        checked={s.notify_sound !== "0"}
                        disabled={s.notify_master === "0"}
                        onCheckedChange={(checked) => patch.mutate({ notify_sound: checked ? "1" : "0" })}
                      />
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title="Tipos de notificação">
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
                    <SettingRow label="Versão">
                      <span className="text-xs text-[#8A8F9C]">0.1.0</span>
                    </SettingRow>
                    <SettingRow label="Stack">
                      <span className="text-xs text-[#8A8F9C]">Tauri v2 · Rust · React</span>
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup title="Créditos">
                    <div className="flex flex-col gap-3 px-[18px] py-4 text-xs leading-relaxed text-[#8A8F9C]">
                      <p>O Torii é feito em cima de projetos abertos e serviços gratuitos:</p>
                      <ul className="flex flex-col gap-1.5">
                        {CREDITS.map((c) => (
                          <li key={c.name}>
                            <span className="font-semibold text-[#C7CAD3]">{c.name}</span>
                            {" · "}
                            {c.role}
                            {c.license && <span className="text-[#6C7180]"> ({c.license})</span>}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </SettingsGroup>

                  <SettingsGroup title="Aviso legal">
                    <div className="px-[18px] py-4 text-xs leading-relaxed text-[#8A8F9C]">
                      O Torii não hospeda, distribui nem armazena nenhum arquivo de mídia. É um
                      gerenciador que automatiza busca e download de torrents através de protocolos
                      públicos (BitTorrent) e organiza o que você mesmo baixa. O conteúdo obtido é de
                      responsabilidade exclusiva de quem usa o app. Respeite as leis de direitos
                      autorais da sua região.
                    </div>
                  </SettingsGroup>
                </>
              )}
            </>
          )}

          <p className="text-[11px] text-[#4E5361]">
            As alterações são salvas automaticamente.
            {patch.isPending && " Salvando…"}
            {savedAt && !patch.isPending && " Salvo."}
          </p>
        </div>
      </div>
    </div>
  );
}
