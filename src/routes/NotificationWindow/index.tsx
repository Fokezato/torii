import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Info, XCircle } from "lucide-react";
import { playNotificationSound, type NotifyPayload, type ToastVariant } from "@/lib/notify";

const VARIANT_ICON: Record<ToastVariant, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  error: XCircle,
};
const VARIANT_COLOR: Record<ToastVariant, string> = {
  info: "#FF6A45",
  success: "#6FC48A",
  error: "#E5484D",
};

const VISIBLE_MS = 5000;

export default function NotificationWindow() {
  const [current, setCurrent] = useState<NotifyPayload | null>(null);
  const queueRef = useRef<NotifyPayload[]>([]);
  const showingRef = useRef(false);

  function processQueue() {
    const next = queueRef.current.shift();
    if (!next) {
      showingRef.current = false;
      setCurrent(null);
      return;
    }
    showingRef.current = true;
    setCurrent(next);
    if (next.sound !== false) playNotificationSound();
    setTimeout(processQueue, VISIBLE_MS);
  }

  useEffect(() => {
    // Documento próprio dessa janela (webview separada) — o CSS global do
    // app principal deixa o <body> opaco, o que quebraria a transparência
    // da janela flutuante. Só afeta essa janela, não a principal. Margin
    // default do body (8px) + nosso padding somados passavam do tamanho da
    // janela e o webview mostrava scrollbar — zera os dois e trava overflow.
    for (const el of [document.documentElement, document.body]) {
      el.style.background = "transparent";
      el.style.margin = "0";
      el.style.overflow = "hidden";
    }

    const unlisten = listen<NotifyPayload>("notify:show", (event) => {
      queueRef.current.push(event.payload);
      if (!showingRef.current) processQueue();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!current) return null;

  const Icon = VARIANT_ICON[current.variant];
  const color = VARIANT_COLOR[current.variant];

  return (
    <div className="flex h-screen w-screen items-center p-2">
      <div
        className="relative w-full overflow-hidden rounded-[12px] shadow-[0_16px_40px_rgba(0,0,0,0.55)]"
        style={{ border: "1px solid rgba(255,255,255,0.08)" }}
      >
        {current.image && (
          <img
            src={current.image}
            alt=""
            className="absolute inset-0 size-full object-cover"
            style={{ filter: "brightness(0.55)" }}
          />
        )}
        <div
          className="absolute inset-0"
          style={
            current.image
              ? { background: "linear-gradient(100deg, rgba(15,16,21,0.92) 38%, rgba(15,16,21,0.55) 100%)" }
              : { background: "rgba(21, 23, 29, 0.88)", backdropFilter: "blur(20px)", WebkitBackdropFilter: "blur(20px)" }
          }
        />

        <div className="relative flex items-start gap-2.5 p-3.5">
          <span
            className="flex size-7 shrink-0 items-center justify-center rounded-[7px]"
            style={{ background: color }}
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="#0B0C10">
              <path
                fillRule="evenodd"
                clipRule="evenodd"
                d="M2.222 3.372a1 1 0 0 1 1.027-.34c1.078.277 2.167.517 3.257.738c1.852.375 4.015.73 5.494.73s3.642-.355 5.494-.73c1.09-.22 2.178-.461 3.256-.738a1 1 0 0 1 1.144 1.415l-2 4c-.15.3-.446.507-.778.546A91 91 0 0 1 16 9.3v1.366a58 58 0 0 0 3.797-.644a1 1 0 0 1 .406 1.958q-.6.123-1.203.23V18a1 1 0 1 1 0 2h-5a1 1 0 1 1 0-2v-5.095c-.692.059-1.374.095-2 .095s-1.308-.037-2-.095V18a1 1 0 1 1 0 2H5a1 1 0 0 1 0-2v-5.79a49 49 0 0 1-1.203-.23a1 1 0 0 1 .406-1.96l.003.002q1.886.382 3.794.643V9.299a91 91 0 0 1-3.116-.306q-.001 0 0 0a1 1 0 0 1-.778-.546l-2-4a1 1 0 0 1 .116-1.075M12 9.5c.617 0 1.304-.024 2-.062v1.459c-.703.063-1.387.103-2 .103s-1.297-.04-2-.103v-1.46c.696.039 1.383.063 2 .063"
              />
            </svg>
          </span>

          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <div className="flex items-center gap-1.5 text-[11px] font-semibold text-[#8A8F9C]">
              <span>Torii</span>
              <span aria-hidden="true">·</span>
              <span>agora</span>
            </div>
            <span className="truncate text-[13px] font-semibold text-[#EEF0F4]" title={current.title}>
              {current.title}
            </span>
            <span className="line-clamp-2 text-xs text-[#B5B9C4]" title={current.body}>
              {current.body}
            </span>
          </div>

          <Icon className="mt-0.5 size-4 shrink-0" style={{ color }} />
        </div>
      </div>
    </div>
  );
}
