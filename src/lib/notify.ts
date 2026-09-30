import { emit } from "@tauri-apps/api/event";

export type ToastVariant = "info" | "success" | "error";

export interface NotifyPayload {
  title: string;
  body: string;
  variant: ToastVariant;
  image?: string | null;
  sound?: boolean;
}

export function notify(
  title: string,
  body: string,
  variant: ToastVariant = "info",
  image?: string | null,
  sound = true,
): void {
  void emit("notify:show", { title, body, variant, image, sound } satisfies NotifyPayload);
}

export function playNotificationSound(): void {
  try {
    const AudioCtx = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    const ctx = new AudioCtx();
    const now = ctx.currentTime;

    const playTone = (freq: number, start: number, duration: number) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(0, now + start);
      gain.gain.linearRampToValueAtTime(0.18, now + start + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.001, now + start + duration);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(now + start);
      osc.stop(now + start + duration);
    };

    playTone(880, 0, 0.14);
    playTone(1318.5, 0.09, 0.18);
    setTimeout(() => ctx.close(), 500);
  } catch {
  }
}
