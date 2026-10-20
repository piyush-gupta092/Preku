// Pip's colors, all derived from the one body color the user picks (Settings → General).

export interface PipPalette {
  body: string;
  /** Soft darker tone for the underside of the body and feet. */
  shade: string;
  /** Lighter tone for the belly. */
  belly: string;
  /** Dark outline, a deep tint of the body color. */
  outline: string;
  cheek: string;
  eye: string;
  leaf: string;
  leafVein: string;
  highlight: string;
}

export const DEFAULT_MASCOT_COLOR = "#ffb38a";

/** Presets offered in Settings (any #rrggbb also works). */
export const MASCOT_COLORS = [
  { id: "peach", color: "#ffb38a" },
  { id: "lemon", color: "#ffd86b" },
  { id: "mint", color: "#9be3b4" },
  { id: "sky", color: "#9cc8ff" },
  { id: "lavender", color: "#c6b2ff" },
  { id: "bubblegum", color: "#ffa6c9" },
  { id: "cocoa", color: "#c99a78" },
  { id: "snow", color: "#f4f1ea" },
] as const;

type Rgb = [number, number, number];

function parse(hex: string): Rgb {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  const value = parseInt(match ? match[1]! : DEFAULT_MASCOT_COLOR.slice(1), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function mix(a: Rgb, b: Rgb, amount: number): Rgb {
  return [0, 1, 2].map((i) => Math.round(a[i]! + (b[i]! - a[i]!) * amount)) as Rgb;
}

const css = ([r, g, b]: Rgb, alpha = 1) =>
  alpha === 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${alpha})`;

export function paletteFor(color: string): PipPalette {
  const body = parse(color);
  const ink: Rgb = [42, 22, 12];
  // Pinkish bodies get a deeper cheek so the blush still shows.
  const cheek = mix([255, 122, 122], body, 0.15);
  return {
    body: css(body),
    shade: css(mix(body, ink, 0.35), 0.35),
    belly: css(mix(body, [255, 255, 255], 0.45)),
    outline: css(mix(body, ink, 0.82)),
    cheek: css(cheek),
    eye: "#1a1423",
    leaf: "#6cc95c",
    leafVein: "#4aa53c",
    highlight: "#ffffff",
  };
}
