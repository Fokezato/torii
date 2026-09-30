import { useEffect, useRef, useState, type RefObject } from "react";
import { playerAmbientFrame } from "@/lib/player";
import type { AmbientSettings } from "@/lib/ambient";

const FRAME_INTERVAL_MS = 60;

export function AmbientLight({
  slotRef,
  aspectRef,
  settings,
}: {
  slotRef: RefObject<HTMLDivElement | null>;
  aspectRef: RefObject<number | null>;
  settings: AmbientSettings;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [slot, setSlot] = useState({ width: 0, height: 0 });
  const [aspect, setAspect] = useState<number | null>(null);
  const blendRef = useRef(1);
  blendRef.current = 1 - (settings.smoothness / 100) * 0.9;

  useEffect(() => {
    const el = slotRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setSlot({ width: el.clientWidth, height: el.clientHeight }));
    observer.observe(el);
    return () => observer.disconnect();
  }, [slotRef]);

  useEffect(() => {
    let stopped = false;
    const scratch = document.createElement("canvas");
    (async () => {
      while (!stopped) {
        const started = performance.now();
        const frame = await playerAmbientFrame().catch(() => null);
        if (stopped) break;
        if (frame && frame.videoWidth > 0 && frame.videoHeight > 0) {
          const next = frame.videoWidth / frame.videoHeight;
          if (aspectRef.current !== next) {
            aspectRef.current = next;
            setAspect(next);
          }
        }
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext("2d");
        const scratchCtx = scratch.getContext("2d");
        if (frame && canvas && ctx && scratchCtx) {
          const { width, height } = frame.pixels;
          if (scratch.width !== width || scratch.height !== height) {
            scratch.width = width;
            scratch.height = height;
          }
          scratchCtx.putImageData(frame.pixels, 0, 0);
          if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
            ctx.globalAlpha = 1;
          } else {
            ctx.globalAlpha = blendRef.current;
          }
          ctx.drawImage(scratch, 0, 0);
        }
        const wait = frame ? FRAME_INTERVAL_MS - (performance.now() - started) : 500;
        if (wait > 0) await new Promise((r) => setTimeout(r, wait));
      }
    })();
    return () => {
      stopped = true;
      aspectRef.current = null;
    };
  }, [aspectRef]);

  const spread = settings.size / 100;
  let box = { left: 0, top: 0, width: 0, height: 0 };
  if (aspect && slot.width > 0 && slot.height > 0) {
    const width = Math.min(slot.width, slot.height * aspect);
    const height = Math.min(slot.height, slot.width / aspect);
    box = {
      width: width * spread,
      height: height * spread,
      left: (slot.width - width * spread) / 2,
      top: (slot.height - height * spread) / 2,
    };
  }

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden>
      <canvas
        ref={canvasRef}
        className="absolute transition-opacity duration-500"
        style={{
          ...box,
          opacity: aspect ? settings.intensity / 100 : 0,
          filter: "blur(64px) saturate(1.35) brightness(0.95)",
        }}
      />
      <div
        className="absolute inset-0"
        style={{ background: "radial-gradient(ellipse at center, transparent 45%, rgba(0,0,0,0.75) 100%)" }}
      />
    </div>
  );
}
