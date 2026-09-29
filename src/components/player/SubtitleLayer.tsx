import { useEffect, useRef, useState } from "react";
import type { SubtitleCue } from "@/lib/player";
import { SUBTITLE_FONT_FAMILY, type SubtitleStyle } from "@/lib/subtitleStyle";

/// Espessura do traço do contorno por nível (0 nenhum … 3 grosso), em em —
/// acompanha o tamanho da fonte. Metade fica sob o preenchimento (ver
/// `paint-order`), então o contorno visível é metade disso.
const OUTLINE_EM = [0, 0.1, 0.16, 0.24];
/// Quanto a legenda sobe quando a barra de controles aparece.
const CONTROLS_LIFT_PX = 96;

/// Contorno de verdade (traço), desenhado ATRÁS do preenchimento — antes era
/// um anel de 8 sombras deslocadas, que no contorno grosso deixava "degraus"
/// entre as direções (parecia a letra cortada).
function outlineStyle(level: number): React.CSSProperties {
  const w = OUTLINE_EM[level] ?? 0;
  return {
    WebkitTextStroke: w > 0 ? `${w}em #000` : undefined,
    paintOrder: "stroke fill",
    textShadow: "0 0.04em 0.18em rgba(0,0,0,0.75)",
  };
}

/// Legenda desenhada pelo Torii por cima do vídeo (modo personalizado).
/// O player é consultado a cada ~400ms; entre uma consulta e outra o tempo
/// é estimado pelo relógio, pra fala não entrar atrasada.
export function SubtitleLayer({
  cues,
  positionMs,
  playing,
  style,
  controlsVisible,
}: {
  cues: SubtitleCue[];
  /** Posição da última consulta ao player. */
  positionMs: number;
  playing: boolean;
  style: SubtitleStyle;
  controlsVisible: boolean;
}) {
  const clockRef = useRef({ pos: positionMs, at: performance.now(), playing });
  // Reancora o relógio só quando chega posição nova do player — não a cada
  // renderização (a própria legenda renderiza várias vezes por segundo).
  useEffect(() => {
    clockRef.current = { pos: positionMs, at: performance.now(), playing };
  }, [positionMs, playing]);
  const [now, setNow] = useState(positionMs);

  useEffect(() => {
    let frame = 0;
    const tick = () => {
      const c = clockRef.current;
      const t = c.playing ? c.pos + (performance.now() - c.at) : c.pos;
      // Arredonda pra não renderizar a cada quadro à toa.
      setNow((prev) => (Math.abs(prev - t) >= 40 ? t : prev));
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);

  const active = cues.filter((c) => !c.sign && c.start_ms <= now && now < c.end_ms);
  if (active.length === 0) return null;

  const textStyle: React.CSSProperties = {
    fontFamily: SUBTITLE_FONT_FAMILY[style.font],
    fontSize: `calc(4.4vh * ${style.size / 100})`,
    fontWeight: 600,
    lineHeight: 1.25,
    color: style.color,
    ...outlineStyle(style.outline),
    whiteSpace: "pre-line",
    backgroundColor: style.background > 0 ? `rgba(0,0,0,${style.background / 100})` : undefined,
    padding: style.background > 0 ? "0.08em 0.35em" : undefined,
    borderRadius: style.background > 0 ? "0.2em" : undefined,
    boxDecorationBreak: "clone",
    WebkitBoxDecorationBreak: "clone",
  };

  const bottom = active.filter((c) => !c.top);
  const top = active.filter((c) => c.top);

  return (
    <div className="pointer-events-none absolute inset-0 z-[5]" aria-live="polite">
      {top.length > 0 && (
        <div className="absolute inset-x-0 top-[6%] flex flex-col items-center gap-1 px-[8%] text-center">
          {top.map((c, i) => (
            <span key={`${c.start_ms}-${i}`} style={{ ...textStyle, fontStyle: c.italic ? "italic" : undefined }}>
              {c.text}
            </span>
          ))}
        </div>
      )}
      {bottom.length > 0 && (
        <div
          className="absolute inset-x-0 flex flex-col items-center gap-1 px-[8%] text-center transition-[bottom] duration-200"
          style={{ bottom: `calc(${style.position}% + ${controlsVisible ? CONTROLS_LIFT_PX : 0}px)` }}
        >
          {bottom.map((c, i) => (
            <span key={`${c.start_ms}-${i}`} style={{ ...textStyle, fontStyle: c.italic ? "italic" : undefined }}>
              {c.text}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
