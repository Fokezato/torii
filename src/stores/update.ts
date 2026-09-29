import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type Phase = "idle" | "checking" | "available" | "downloading" | "error" | "upToDate";

interface UpdateState {
  phase: Phase;
  update: Update | null;
  /** 0–100 durante o download. */
  progress: number;
  error: string | null;
  /** "Depois" — some até a próxima checagem manual ou reabrir o app. */
  dismissed: boolean;
  /** Procura versão nova na última release do GitHub. */
  checkNow: () => Promise<void>;
  /** Baixa, instala e reinicia o Torii. */
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  phase: "idle",
  update: null,
  progress: 0,
  error: null,
  dismissed: false,

  checkNow: async () => {
    if (get().phase === "checking" || get().phase === "downloading") return;
    set({ phase: "checking", error: null, dismissed: false });
    try {
      const update = await check();
      set(update ? { phase: "available", update } : { phase: "upToDate", update: null });
    } catch (e) {
      set({ phase: "error", error: String(e) });
    }
  },

  install: async () => {
    const update = get().update;
    if (!update) return;
    set({ phase: "downloading", progress: 0, error: null });
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          done += event.data.chunkLength;
          if (total > 0) set({ progress: Math.min(100, (done / total) * 100) });
        }
      });
      await relaunch();
    } catch (e) {
      set({ phase: "error", error: String(e) });
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
