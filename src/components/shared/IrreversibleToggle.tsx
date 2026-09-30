import { useState } from "react";
import { TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
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

export type ReduceSizeFeature = "stripAudio" | "downscale";

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
  scope: "global" | "anime";
}) {
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  const label = t(`reduceSize.${feature}.label`);

  return (
    <>
      <div className="flex items-center justify-between gap-5">
        <div className="flex flex-col gap-0.5">
          <span className="flex items-center gap-2 text-[13px] font-semibold">
            {label}
            <span className="rounded-full border border-primary/40 px-1.5 py-0.5 text-[9px] font-bold tracking-wide text-primary uppercase">
              {t("common.beta")}
            </span>
          </span>
          <span className="text-[11.5px] text-[#6C7180]">
            {forcedOn ? t("reduceSize.forcedOn") : t(`reduceSize.${feature}.description`)}
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
              {t("reduceSize.confirmTitle", { feature: label })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(`reduceSize.${feature}.warning`)}{" "}
              {scope === "global" ? t("reduceSize.scopeGlobal") : t("reduceSize.scopeAnime")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                onCheckedChange(true);
                setConfirming(false);
              }}
            >
              {t("common.confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
