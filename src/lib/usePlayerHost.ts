import { useEffect, type RefObject } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit, listen } from "@tauri-apps/api/event";
import { playerSetVideoArea } from "@/lib/player";

export function usePlayerHost(
  slotRef: RefObject<HTMLDivElement | null>,
  onError?: (message: string) => void,
  videoAspectRef?: RefObject<number | null>,
) {
  const navigate = useNavigate();

  useEffect(() => {
    const unlisten = listen("player:back-requested", () => navigate(-1));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [navigate]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      const target = e.target as HTMLElement | null;
      if (target && ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName)) return;
      if (e.key === " " || e.key.startsWith("Arrow")) e.preventDefault();
      emit("player:shortcut", { key: e.key, shift: e.shiftKey }).catch(() => {});
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const unlisten = listen("player:toggle-fullscreen-requested", async () => {
      const win = getCurrentWindow();
      const isFull = await win.isFullscreen().catch(() => false);
      await win.setFullscreen(!isFull).catch((e) => onError?.(String(e)));
    });
    const unlistenExit = listen("player:exit-fullscreen-requested", () => {
      getCurrentWindow()
        .setFullscreen(false)
        .catch(() => {});
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenExit.then((fn) => fn());
      getCurrentWindow()
        .isFullscreen()
        .then((full) => (full ? getCurrentWindow().setFullscreen(false) : undefined))
        .catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const el = slotRef.current;
    if (!el) return;
    let cancelled = false;

    const reportBounds = async () => {
      const winPos = await getCurrentWindow().innerPosition();
      if (cancelled) return;
      const rect = el.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = Math.round(rect.left * dpr);
      const y = Math.round(rect.top * dpr);
      const width = Math.round(rect.width * dpr);
      const height = Math.round(rect.height * dpr);
      const aspect = videoAspectRef?.current;
      let video: [number, number, number, number] | undefined;
      if (aspect && width > 1 && height > 1) {
        const fitW = Math.min(width, Math.round(height * aspect));
        const fitH = Math.min(height, Math.round(width / aspect));
        video = [x + Math.round((width - fitW) / 2), y + Math.round((height - fitH) / 2), fitW, fitH];
      }
      playerSetVideoArea(x, y, width, height, winPos.x + x, winPos.y + y, video).catch((e) => onError?.(String(e)));
    };

    reportBounds();
    const observer = new ResizeObserver(reportBounds);
    observer.observe(el);
    window.addEventListener("resize", reportBounds);
    document.addEventListener("scroll", reportBounds, true);
    const unlistenMoved = getCurrentWindow().onMoved(() => reportBounds());
    const resyncInterval = setInterval(reportBounds, 1000);

    return () => {
      cancelled = true;
      observer.disconnect();
      window.removeEventListener("resize", reportBounds);
      document.removeEventListener("scroll", reportBounds, true);
      unlistenMoved.then((fn) => fn());
      clearInterval(resyncInterval);
      playerSetVideoArea(-2000, -2000, 1, 1, -2000, -2000).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
