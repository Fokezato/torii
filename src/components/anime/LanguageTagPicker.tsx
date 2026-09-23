import { useState } from "react";
import { Check, Plus, X } from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { LANGUAGES } from "@/lib/constants";

const ALL_OPTIONS = LANGUAGES.filter((l) => l.value !== "any");

interface LanguageTagPickerProps {
  selected: string[];
  onChange: (values: string[]) => void;
  /**
   * Idiomas que de fato apareceram em torrents desse anime (ver
   * nyaa_available_languages). Quando presente e não-vazio, a lista mostra
   * só essas opções — não faz sentido oferecer 17 idiomas genéricos se só
   * 3 têm torrent de verdade, isso só induz a escolher algo que não existe.
   * `undefined`/vazio = ainda carregando ou detecção falhou: cai pra lista
   * genérica completa em vez de mostrar um seletor vazio.
   */
  available?: string[];
}

export function LanguageTagPicker({ selected, onChange, available }: LanguageTagPickerProps) {
  const [open, setOpen] = useState(false);
  const OPTIONS =
    available && available.length > 0 ? ALL_OPTIONS.filter((o) => available.includes(o.value)) : ALL_OPTIONS;

  function toggle(value: string) {
    if (selected.includes(value)) onChange(selected.filter((v) => v !== value));
    else onChange([...selected, value]);
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {selected.length === 0 && <span className="text-xs text-[#6C7180]">Qualquer</span>}
      {selected.map((value) => {
        const label = OPTIONS.find((o) => o.value === value)?.label ?? value;
        return (
          <span
            key={value}
            className="flex items-center gap-1 rounded-full border border-[#262A35] bg-[#1B1E27] py-1 pr-1.5 pl-2.5 text-xs"
          >
            {label}
            <button
              type="button"
              onClick={() => toggle(value)}
              aria-label={`Remover ${label}`}
              className="flex size-3.5 items-center justify-center rounded-full text-[#6C7180] hover:text-foreground"
            >
              <X className="size-3" />
            </button>
          </span>
        );
      })}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            aria-label="Adicionar idioma"
            className="flex size-6 items-center justify-center rounded-full border border-[#262A35] bg-[#1B1E27] text-[#9BA0AE] transition-colors hover:text-foreground"
          >
            <Plus className="size-3.5" />
          </button>
        </PopoverTrigger>
        <PopoverContent align="start" className="max-h-64 w-56 overflow-y-auto p-1.5">
          {OPTIONS.map((o) => {
            const active = selected.includes(o.value);
            return (
              <button
                key={o.value}
                type="button"
                onClick={() => toggle(o.value)}
                className={`flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-left text-sm transition-colors ${
                  active ? "bg-secondary text-primary" : "hover:bg-white/5"
                }`}
              >
                {o.label}
                {active && <Check className="size-3.5" />}
              </button>
            );
          })}
        </PopoverContent>
      </Popover>
    </div>
  );
}
