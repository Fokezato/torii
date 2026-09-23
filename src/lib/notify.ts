import { emit } from "@tauri-apps/api/event";

export type ToastVariant = "info" | "success" | "error";

export interface NotifyPayload {
  title: string;
  body: string;
  variant: ToastVariant;
  image?: string | null;
  sound?: boolean;
}

/**
 * Notificação com visual de toast do Windows, mas não é notificação nativa
 * do Windows — é uma janela Tauri própria (sem borda, sempre no topo, some
 * da taskbar), porque o toast nativo (tauri-plugin-notification/WinRT) exige
 * AUMID registrado e continuou sendo descartado silenciosamente mesmo no
 * app instalado de verdade nessa máquina. Emite pro evento que a janela
 * "notification-window" escuta — ver src/routes/NotificationWindow.
 */
export function notify(
  title: string,
  body: string,
  variant: ToastVariant = "info",
  image?: string | null,
  sound = true,
): void {
  void emit("notify:show", { title, body, variant, image, sound } satisfies NotifyPayload);
}

/**
 * Som sintetizado (dois tons rápidos), sem depender de arquivo de áudio
 * embutido. Chamado pela janela de notificação quando o payload pede som.
 */
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
    // Web Audio indisponível — silencioso, não impede o toast de aparecer.
  }
}
