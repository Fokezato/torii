import { useEffect, type RefObject } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit, listen } from "@tauri-apps/api/event";
import { playerSetVideoArea } from "@/lib/player";

/// Liga uma página ao player nativo: posiciona a HWND do vídeo (e a janela
/// de controles por cima) em cima do elemento `slotRef`, e trata os eventos
/// que a overlay emite ("voltar", "tela cheia"). Ao sair da página, some
/// com o vídeo/overlay — a visibilidade é derivada da área no Rust (área
/// fora da tela = oculto), ver `player_set_video_area`.
export function usePlayerHost(slotRef: RefObject<HTMLDivElement | null>, onError?: (message: string) => void) {
  const navigate = useNavigate();

  useEffect(() => {
    const unlisten = listen("player:back-requested", () => navigate(-1));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [navigate]);

  // Atalhos de teclado com o foco nessa janela: repassa pra overlay, que
  // tem o estado do player e trata tudo num lugar só (ver PlayerOverlay).
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
      // Saiu do player em tela cheia — volta a janela ao normal.
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

    // Lê a posição da JANELA primeiro (lado lento, round-trip IPC) e só
    // DEPOIS o rect do elemento (síncrono) — minimiza a janela de tempo
    // entre as 2 leituras (drift durante resize/move contínuo "vazava" a
    // overlay pra fora da janela — bug real reportado).
    const reportBounds = async () => {
      const winPos = await getCurrentWindow().innerPosition();
      if (cancelled) return;
      const rect = el.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = Math.round(rect.left * dpr);
      const y = Math.round(rect.top * dpr);
      const width = Math.round(rect.width * dpr);
      const height = Math.round(rect.height * dpr);
      playerSetVideoArea(x, y, width, height, winPos.x + x, winPos.y + y).catch((e) => onError?.(String(e)));
    };

    reportBounds();
    const observer = new ResizeObserver(reportBounds);
    observer.observe(el);
    window.addEventListener("resize", reportBounds);
    // scroll não borbulha no DOM — captura no document pega scroll de
    // qualquer ancestral rolável.
    document.addEventListener("scroll", reportBounds, true);
    // Mover a janela não muda o viewport — sem isso a overlay ficava parada.
    const unlistenMoved = getCurrentWindow().onMoved(() => reportBounds());
    // Rede de segurança: resincroniza a cada 1s (corrige drift residual e
    // qualquer gatilho perdido, ex. maximizar).
    const resyncInterval = setInterval(reportBounds, 1000);

    return () => {
      cancelled = true;
      observer.disconnect();
      window.removeEventListener("resize", reportBounds);
      document.removeEventListener("scroll", reportBounds, true);
      unlistenMoved.then((fn) => fn());
      clearInterval(resyncInterval);
      // Overlay vive a vida toda do app (ver spawn_player_overlay_window) —
      // só manda ela e o vídeo pra fora da tela (= ocultos).
      playerSetVideoArea(-2000, -2000, 1, 1, -2000, -2000).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
