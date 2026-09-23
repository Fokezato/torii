import { useState } from "react";
import { TriangleAlert } from "lucide-react";
import { Switch } from "@/components/ui/switch";
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

export type ReduceSizeFeature = "strip_audio" | "downscale";

export const REDUCE_SIZE_FEATURES: Record<
  ReduceSizeFeature,
  { label: string; description: string; warning: string }
> = {
  strip_audio: {
    label: "Remover áudios extras",
    description: "Deixa só os áudios nos idiomas preferidos (Reprodução) e o japonês original. Sem perder qualidade.",
    warning:
      "Os áudios em outros idiomas são apagados de dentro do arquivo. Pra ter eles de volta, só baixando o episódio de novo.",
  },
  downscale: {
    label: "Reduzir resolução pra 720p",
    description: "Recodifica o vídeo depois de baixar (~3 min por episódio com placa de vídeo, mais no processador).",
    warning:
      "O vídeo original é substituído por uma versão 720p, um pouco menos nítida. Pra ter a qualidade original de volta, só baixando o episódio de novo.",
  },
};

/// Chave de opção que mexe no arquivo sem volta: LIGAR pede confirmação
/// (aviso de irreversível + botão Confirmar); desligar é direto — não
/// desfaz o que já foi processado, só para de processar os próximos.
/// `forcedOn`: ligada na Config (global) — aparece ligada e travada.
export function IrreversibleToggle({
  feature,
  checked,
  onCheckedChange,
  forcedOn = false,
  scope,
}: {
  feature: ReduceSizeFeature;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  forcedOn?: boolean;
  /** "global": vale pra todos os animes; "anime": só esse. Muda o texto do aviso. */
  scope: "global" | "anime";
}) {
  const [confirming, setConfirming] = useState(false);
  const info = REDUCE_SIZE_FEATURES[feature];

  return (
    <>
      <div className="flex items-center justify-between gap-5">
        <div className="flex flex-col gap-0.5">
          <span className="flex items-center gap-2 text-[13px] font-semibold">
            {info.label}
            <span className="rounded-full border border-primary/40 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-primary uppercase">
              Beta
            </span>
          </span>
          <span className="text-[11.5px] text-[#6C7180]">
            {forcedOn ? "Ligado pra todos os animes em Configurações." : info.description}
          </span>
        </div>
        <Switch
          checked={forcedOn || checked}
          disabled={forcedOn}
          onCheckedChange={(next) => (next ? setConfirming(true) : onCheckedChange(false))}
        />
      </div>

      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2">
              <TriangleAlert className="size-5 text-[#E5A34B]" />
              {info.label}: não tem volta
            </AlertDialogTitle>
            <AlertDialogDescription>
              {info.warning}{" "}
              {scope === "global"
                ? "Vale pra TODOS os animes, inclusive os episódios que você já baixou."
                : "Vale pra todos os episódios desse anime, inclusive os que já foram baixados."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancelar</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                onCheckedChange(true);
                setConfirming(false);
              }}
            >
              Confirmar
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
