import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { FolderX, LibraryBig, Trash2, X } from "lucide-react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { removeWatch, type RemoveMode, type Watch } from "@/lib/watches";
import { notify } from "@/lib/notify";

const OPTIONS: { mode: RemoveMode; icon: typeof Trash2 }[] = [
  { mode: "everything", icon: Trash2 },
  { mode: "keep_files", icon: LibraryBig },
  { mode: "files_only", icon: FolderX },
];

/// "Remover" da página do anime: tirar da Biblioteca apagando ou mantendo os
/// arquivos, ou apagar só os arquivos e manter na Biblioteca.
export function RemoveWatchDialog({
  watch,
  onOpenChange,
  onDone,
}: {
  watch: Watch | null;
  onOpenChange: (open: boolean) => void;
  /** Depois de remover — `mode` diz se o anime saiu da Biblioteca. */
  onDone: (mode: RemoveMode) => void;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<RemoveMode>("everything");

  const mutation = useMutation({
    mutationFn: () => removeWatch(watch!.id, mode),
    onSuccess: (failed) => {
      if (failed > 0) notify(t("remove.title"), t("remove.failedFiles", { count: failed }), "error");
      onOpenChange(false);
      onDone(mode);
    },
  });

  return (
    <Dialog open={watch !== null} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        style={{ maxWidth: "520px", width: "92vw" }}
        className="gap-0 overflow-hidden rounded-[20px] bg-[#15171D] p-0 ring-0"
      >
        {watch && (
          <>
            <div className="flex min-w-0 items-center justify-between gap-3 border-b border-[#1E212A] px-[22px] py-[18px]">
              <h2 className="min-w-0 truncate text-base font-bold">
                {t("remove.title")} · {watch.title}
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

            <div role="radiogroup" aria-label={t("remove.title")} className="flex flex-col gap-2.5 p-[22px]">
              {OPTIONS.map(({ mode: option, icon: Icon }) => {
                const selected = mode === option;
                return (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    onClick={() => setMode(option)}
                    className={`flex items-start gap-3 rounded-xl border px-4 py-3.5 text-left transition-colors ${
                      selected
                        ? "border-primary/60 bg-primary/[0.07]"
                        : "border-[#262A35] bg-[#1B1E27] hover:border-[#33374A]"
                    }`}
                  >
                    <Icon className={`mt-0.5 size-4 shrink-0 ${selected ? "text-primary" : "text-[#6C7180]"}`} />
                    <span className="flex flex-col gap-0.5">
                      <span className="text-[13px] font-semibold">{t(`remove.${option}.label`)}</span>
                      <span className="text-xs text-[#6C7180]">{t(`remove.${option}.description`)}</span>
                    </span>
                  </button>
                );
              })}
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
                className="rounded-[10px] bg-[#B5576B] px-5 py-2.5 text-[13px] font-semibold text-white transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {mutation.isPending ? t("remove.removing") : t(`remove.${mode}.confirm`)}
              </button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
