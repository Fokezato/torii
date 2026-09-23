import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, X } from "lucide-react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Slider } from "@/components/ui/slider";
import { LanguageTagPicker } from "@/components/anime/LanguageTagPicker";
import { QUALITIES } from "@/lib/constants";
import { createWatch } from "@/lib/watches";
import { getAvailableLanguages } from "@/lib/nyaa";
import type { AnimeSummary } from "@/lib/anilist";
import { getSettings } from "@/lib/tauri";
import { ffmpegInstall } from "@/lib/tools";
import { IrreversibleToggle } from "@/components/shared/IrreversibleToggle";

interface WatchFormModalProps {
  anime: AnimeSummary | null;
  onOpenChange: (open: boolean) => void;
}

type Tab = "local" | "streaming";

function DropdownSelect({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (v: string) => void;
  options: ReadonlyArray<{ value: string; label: string }>;
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger
        size="sm"
        style={{ backgroundColor: "#1B1E27", borderColor: "#262A35" }}
        className="rounded-lg px-3 text-xs"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function WatchFormModal({ anime, onOpenChange }: WatchFormModalProps) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<Tab>("local");
  const [quality, setQuality] = useState("any");
  const [audioLangs, setAudioLangs] = useState<string[]>([]);
  const [subLangs, setSubLangs] = useState<string[]>([]);
  const [autoDownload, setAutoDownload] = useState(true);
  const [notify, setNotify] = useState(true);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [stripAudio, setStripAudio] = useState(false);
  const [downscale, setDownscale] = useState(false);
  // Ligado na Config = vale pra todos; aqui só aparece travado.
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: getSettings, enabled: anime != null });
  const globalStrip = settings?.strip_unused_audio === "1";
  const globalDownscale = settings?.downscale_resolution === "720p";
  const totalEpisodes = anime?.episodes ?? 24;
  const [episodeRange, setEpisodeRange] = useState<[number, number]>([1, totalEpisodes]);
  const rangeIsFull = episodeRange[0] === 1 && episodeRange[1] === totalEpisodes;

  const { data: availableLangs } = useQuery({
    queryKey: ["nyaa-languages", anime?.title],
    queryFn: () => getAvailableLanguages(anime!.title),
    enabled: anime != null,
    staleTime: 10 * 60_000,
  });

  useEffect(() => {
    if (anime) {
      setTab("local");
      setQuality("any");
      setAudioLangs([]);
      setSubLangs([]);
      setAutoDownload(true);
      setNotify(true);
      setAdvancedOpen(false);
      setStripAudio(false);
      setDownscale(false);
      setEpisodeRange([1, anime.episodes ?? 24]);
    }
  }, [anime]);

  const mutation = useMutation({
    mutationFn: () =>
      createWatch({
        title: anime!.title,
        query: anime!.title,
        quality,
        audio_lang: audioLangs.length ? audioLangs.join(",") : null,
        sub_lang: subLangs.length ? subLangs.join(",") : null,
        anilist_id: anime!.anilist_id,
        cover_url: anime!.cover_url,
        status: anime!.status,
        episodes: anime!.episodes,
        active: autoDownload,
        notify_on_available: notify,
        episode_start: rangeIsFull ? null : episodeRange[0],
        episode_end: rangeIsFull ? null : episodeRange[1],
        strip_audio: stripAudio,
        max_resolution: downscale ? "720p" : null,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["watches"] });
      // Já deixa o ffmpeg baixando, pra estar pronto quando o 1º episódio terminar.
      if (stripAudio || downscale) ffmpegInstall().catch(() => {});
      onOpenChange(false);
    },
  });

  return (
    <Dialog open={anime !== null} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "600px", width: "92vw" }}
        className="gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
      >
        {anime && (
          <>
            <div className="flex items-center gap-3.5 border-b border-[#1E212A] px-[22px] py-[18px]">
              <div className="h-[65px] w-[46px] shrink-0 overflow-hidden rounded-lg bg-secondary">
                {anime.cover_url && (
                  <img src={anime.cover_url} alt="" className="h-full w-full object-cover" />
                )}
              </div>
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="text-[10px] font-bold tracking-wide text-[#6C7180] uppercase">
                  Adicionar à biblioteca
                </span>
                <h2 className="truncate text-base font-bold">{anime.title}</h2>
              </div>
              <button
                type="button"
                aria-label="Fechar"
                onClick={() => onOpenChange(false)}
                className="ml-auto flex size-[30px] shrink-0 items-center justify-center rounded-lg text-[#6C7180] transition-colors hover:bg-white/5 hover:text-foreground"
              >
                <X className="size-[15px]" strokeWidth={2} />
              </button>
            </div>

            <div
              role="tablist"
              aria-label="Tipo de disponibilização"
              className="flex gap-1 border-b border-[#1E212A] px-[22px]"
            >
              <button
                type="button"
                role="tab"
                aria-selected={tab === "local"}
                onClick={() => setTab("local")}
                className={`-mb-px border-b-2 px-1.5 py-3.5 text-[13px] font-bold transition-colors ${
                  tab === "local"
                    ? "border-primary text-foreground"
                    : "border-transparent text-[#6C7180] hover:text-foreground"
                }`}
              >
                Local
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={tab === "streaming"}
                onClick={() => setTab("streaming")}
                className={`-mb-px border-b-2 px-1.5 py-3.5 text-[13px] font-bold transition-colors ${
                  tab === "streaming"
                    ? "border-primary text-foreground"
                    : "border-transparent text-[#6C7180] hover:text-foreground"
                }`}
              >
                Streaming
              </button>
            </div>

            {tab === "local" ? (
              <div role="tabpanel" className="flex flex-col gap-[18px] p-[22px]">
                <div className="flex items-center justify-between gap-5">
                  <span className="text-[13px] font-semibold">Qualidade</span>
                  <DropdownSelect value={quality} onChange={setQuality} options={QUALITIES} />
                </div>
                <div className="flex flex-col gap-2">
                  <span className="text-[13px] font-semibold">Idioma de áudio</span>
                  <LanguageTagPicker selected={audioLangs} onChange={setAudioLangs} available={availableLangs?.audio} />
                </div>
                <div className="flex flex-col gap-2">
                  <span className="text-[13px] font-semibold">Legenda</span>
                  <LanguageTagPicker selected={subLangs} onChange={setSubLangs} available={availableLangs?.subtitles} />
                </div>
                {availableLangs && availableLangs.audio.length === 0 && availableLangs.subtitles.length === 0 && (
                  <p className="text-[11px] text-[#6C7180]">
                    Não achei idioma marcado nos releases desse anime — mostrando a lista completa.
                  </p>
                )}

                <div className="h-px bg-[#1E212A]" />

                <div className="flex items-center justify-between gap-5">
                  <div className="flex flex-col gap-0.5">
                    <span className="text-[13px] font-semibold">
                      Baixar novos episódios automaticamente
                    </span>
                    <span className="text-[11.5px] text-[#6C7180]">Assim que forem lançados</span>
                  </div>
                  <Switch checked={autoDownload} onCheckedChange={setAutoDownload} />
                </div>
                <div className="flex items-center justify-between gap-5">
                  <span className="text-[13px] font-semibold">
                    Notificar quando um episódio estiver pronto
                  </span>
                  <Switch checked={notify} onCheckedChange={setNotify} />
                </div>

                <div className="h-px bg-[#1E212A]" />

                <div className="flex flex-col gap-3">
                  <button
                    type="button"
                    onClick={() => setAdvancedOpen((v) => !v)}
                    aria-expanded={advancedOpen}
                    className="flex items-center justify-between text-[13px] font-semibold text-foreground"
                  >
                    Avançado
                    <ChevronDown
                      className={`size-4 text-[#6C7180] transition-transform ${advancedOpen ? "rotate-180" : ""}`}
                    />
                  </button>

                  {advancedOpen && (
                    <div className="flex flex-col gap-3">
                      <div className="flex items-center justify-between">
                        <span className="text-[13px] font-semibold">Quais episódios baixar</span>
                        <span className="text-[12px] font-semibold text-primary">
                          {rangeIsFull ? "Todos" : `Ep ${episodeRange[0]} – ${episodeRange[1]}`}
                        </span>
                      </div>
                      <span className="text-[11.5px] text-[#6C7180]">
                        Arraste as pontas pra pegar só um trecho da temporada.
                      </span>
                      <Slider
                        min={1}
                        max={totalEpisodes}
                        step={1}
                        value={episodeRange}
                        onValueChange={(v) => setEpisodeRange([v[0], v[1]])}
                        className="py-1.5"
                      />
                      <div className="flex items-center justify-between text-[11px] text-[#6C7180]">
                        <span>Ep 1</span>
                        <span>Ep {totalEpisodes}</span>
                      </div>

                      <div className="my-1 h-px bg-[#1E212A]" />
                      <span className="text-[13px] font-semibold">Reduzir tamanho dos arquivos</span>
                      <IrreversibleToggle
                        feature="strip_audio"
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
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div role="tabpanel" className="flex flex-col items-center gap-2 p-[22px] py-10 text-center">
                <p className="text-sm font-semibold">Pseudo-stream ainda não existe</p>
                <p className="max-w-xs text-xs text-[#6C7180]">
                  Assistir enquanto baixa e apagar depois fica pra uma fase futura do desenvolvimento.
                  Por enquanto, use o modo Local.
                </p>
              </div>
            )}

            <div className="flex items-center justify-end gap-3 border-t border-[#1E212A] px-[22px] py-[18px]">
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                className="rounded-[10px] border border-[#33374A] px-[18px] py-2.5 text-[13px] font-semibold text-[#9BA0AE] transition-colors hover:text-foreground"
              >
                Cancelar
              </button>
              <button
                type="button"
                disabled={tab === "streaming" || mutation.isPending}
                onClick={() => mutation.mutate()}
                className="rounded-[10px] bg-primary px-5 py-2.5 text-[13px] font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {mutation.isPending ? "Adicionando..." : "Adicionar"}
              </button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
