/// Legenda personalizada (desenhada pelo Torii, ver components/player/
/// SubtitleLayer). Salvo nas configurações como `player_sub_*`.
export type SubtitleFont = "sans" | "serif" | "rounded";

export interface SubtitleStyle {
  /** "original" = legenda do arquivo, desenhada pelo VLC. */
  mode: "original" | "custom";
  /** Tamanho em % (100 = padrão). */
  size: number;
  font: SubtitleFont;
  /** Cor do texto, "#RRGGBB". */
  color: string;
  /** Contorno: 0 nenhum, 1 fino, 2 normal, 3 grosso. */
  outline: number;
  /** Opacidade do fundo atrás do texto, 0–100. */
  background: number;
  /** Altura a partir de baixo, em % da tela (0–25). */
  position: number;
}

export const DEFAULT_SUBTITLE_STYLE: SubtitleStyle = {
  mode: "original",
  size: 100,
  font: "sans",
  color: "#FFFFFF",
  outline: 2,
  background: 0,
  position: 6,
};

export const SUBTITLE_COLORS = ["#FFFFFF", "#FFFF00", "#7FE3FF", "#A6FF8F", "#FF9CD2"];

export const SUBTITLE_FONT_FAMILY: Record<SubtitleFont, string> = {
  sans: "'Segoe UI', Arial, sans-serif",
  serif: "Georgia, 'Times New Roman', serif",
  rounded: "'Trebuchet MS', 'Segoe UI', sans-serif",
};

export function subtitleStyleFromSettings(s: Record<string, string>): SubtitleStyle {
  const d = DEFAULT_SUBTITLE_STYLE;
  const num = (key: string, fallback: number, min: number, max: number) => {
    const v = Number(s[key]);
    return s[key] != null && s[key] !== "" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback;
  };
  const font = s.player_sub_font;
  return {
    mode: s.player_sub_mode === "custom" ? "custom" : "original",
    size: num("player_sub_size", d.size, 50, 250),
    font: font === "serif" || font === "rounded" ? font : "sans",
    // Amarelo antigo (pastel) vira o amarelo forte.
    color: /^#[0-9a-fA-F]{6}$/.test(s.player_sub_color ?? "")
      ? s.player_sub_color.toUpperCase() === "#FFE14D"
        ? "#FFFF00"
        : s.player_sub_color
      : d.color,
    outline: num("player_sub_outline", d.outline, 0, 3),
    background: num("player_sub_background", d.background, 0, 100),
    position: num("player_sub_position", d.position, 0, 25),
  };
}

export function subtitleStyleToSettings(st: SubtitleStyle): Record<string, string> {
  return {
    player_sub_mode: st.mode,
    player_sub_size: String(st.size),
    player_sub_font: st.font,
    player_sub_color: st.color,
    player_sub_outline: String(st.outline),
    player_sub_background: String(st.background),
    player_sub_position: String(st.position),
  };
}
