import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { CalendarClock, Eye } from "lucide-react";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

export interface DeletionRulesValue {
  /** Days after download; `null` = off. */
  days: number | null;
  afterWatched: boolean;
}

const DEFAULT_DAYS = 7;
const MAX_DAYS = 90;

function RuleCard({
  icon,
  title,
  description,
  checked,
  disabled,
  onCheckedChange,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
  children?: ReactNode;
}) {
  return (
    <div className={cn("flex flex-col gap-3 rounded-[10px] border border-[#262A35] bg-[#1B1E27] p-3.5", disabled && "opacity-60")}>
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-[#8A8F9C]">{icon}</span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="text-[13px] font-semibold">{title}</span>
          <span className="text-[11.5px] text-[#6C7180]">{description}</span>
        </div>
        <Switch checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
      </div>
      {children}
    </div>
  );
}

/// The two ways Torii deletes episodes on its own, same UI in Settings and in an anime's preferences.
export function DeletionRules({
  value,
  onChange,
  disabled,
  jellyfinUnset,
}: {
  value: DeletionRulesValue;
  onChange: (value: DeletionRulesValue) => void;
  disabled?: boolean;
  jellyfinUnset?: boolean;
}) {
  const { t } = useTranslation();
  const [days, setDays] = useState(value.days ?? DEFAULT_DAYS);
  useEffect(() => {
    if (value.days != null) setDays(value.days);
  }, [value.days]);

  return (
    <div className="flex flex-col gap-2.5">
      <RuleCard
        icon={<CalendarClock className="size-4" />}
        title={t("deletion.byTime")}
        description={t("deletion.byTimeHint")}
        checked={value.days != null}
        disabled={disabled}
        onCheckedChange={(on) => onChange({ ...value, days: on ? days : null })}
      >
        {value.days != null && (
          <div className="flex items-center gap-3 pl-7">
            <Slider
              min={1}
              max={MAX_DAYS}
              step={1}
              value={[days]}
              disabled={disabled}
              onValueChange={([v]) => setDays(v)}
              onValueCommit={([v]) => onChange({ ...value, days: v })}
              className="flex-1 py-1.5"
            />
            <span className="w-24 shrink-0 text-right text-[12px] font-semibold text-primary">
              {t("deletion.days", { count: days })}
            </span>
          </div>
        )}
      </RuleCard>
      <RuleCard
        icon={<Eye className="size-4" />}
        title={t("deletion.afterWatched")}
        description={t("deletion.afterWatchedHint")}
        checked={value.afterWatched}
        disabled={disabled}
        onCheckedChange={(afterWatched) => onChange({ ...value, afterWatched })}
      >
        {value.afterWatched && jellyfinUnset && (
          <p className="pl-7 text-[11px] text-[#8A8F9C]">{t("deletion.jellyfinOff")}</p>
        )}
      </RuleCard>
    </div>
  );
}

export function rulesFromSettings(s: Record<string, string> | undefined): DeletionRulesValue {
  const days = Number(s?.default_delete_after_days);
  return { days: days > 0 ? days : null, afterWatched: s?.delete_after_watched === "1" };
}
