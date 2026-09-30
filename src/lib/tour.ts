import { driver } from "driver.js";
import "driver.js/dist/driver.css";
import { t } from "@/i18n";

const STEPS = [
  { anchor: "nav-home", key: "home", side: "right" },
  { anchor: "hero", key: "add", side: "bottom" },
  { anchor: "nav-explore", key: "explore", side: "right" },
  { anchor: "nav-library", key: "library", side: "right" },
  { anchor: "nav-downloads", key: "downloads", side: "right" },
  { anchor: "activity", key: "activity", side: "bottom" },
  { anchor: "nav-settings", key: "settings", side: "right" },
] as const;

/// Guided tour over the main screens. Starts on Home (the anchors live there).
export function startTour(onDone?: () => void) {
  const tour = driver({
    showProgress: true,
    allowClose: true,
    overlayOpacity: 0.72,
    stagePadding: 6,
    stageRadius: 12,
    popoverClass: "torii-tour",
    progressText: t("tour.progress", { current: "{{current}}", total: "{{total}}" }),
    nextBtnText: t("tour.next"),
    prevBtnText: t("tour.back"),
    doneBtnText: t("tour.done"),
    steps: STEPS.filter((s) => document.querySelector(`[data-tour="${s.anchor}"]`)).map((s) => ({
      element: `[data-tour="${s.anchor}"]`,
      popover: {
        title: t(`tour.steps.${s.key}.title`),
        description: t(`tour.steps.${s.key}.text`),
        side: s.side,
        align: "start",
      },
    })),
    onDestroyed: () => onDone?.(),
  });
  tour.drive();
}

/// Other screens (Settings) ask the shell to go Home and start the tour.
export const START_TOUR_EVENT = "torii:start-tour";

export function requestTour() {
  window.dispatchEvent(new Event(START_TOUR_EVENT));
}
