// Farbrechnung für Tokens, Kontrasttest und Akzentableitung (UI-120). Reine Funktionen ohne DOM.

export type Rgb = readonly [number, number, number]; // 0 … 255

/** Liest `#rgb` oder `#rrggbb`; wirft bei ungültiger Eingabe. */
export function parseHex(hex: string): Rgb {
  const h = hex.trim().replace(/^#/, "");
  const full = h.length === 3 ? [...h].map((c) => c + c).join("") : h;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) throw new Error(`ungültige Farbe: ${hex}`);
  return [parseInt(full.slice(0, 2), 16), parseInt(full.slice(2, 4), 16), parseInt(full.slice(4, 6), 16)];
}

export function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value.trim());
}

export function toHex(rgb: Rgb): string {
  return (
    "#" +
    rgb
      .map((c) => Math.max(0, Math.min(255, Math.round(c))).toString(16).padStart(2, "0"))
      .join("")
      .toUpperCase()
  );
}

function linear(c: number): number {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

/** Relative Luminanz nach WCAG 2.x. */
export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex);
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** Kontrastverhältnis nach WCAG 2.x (1 … 21). */
export function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const [hi, lo] = la >= lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

// --- OKLab-Mischung (gleiche Rechnung für CSS-Variablen und Canvas-Theme) --------------------------------------

function toOklab(rgb: Rgb): [number, number, number] {
  const r = linear(rgb[0]);
  const g = linear(rgb[1]);
  const b = linear(rgb[2]);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function fromOklab([L, a, b]: [number, number, number]): Rgb {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const lin: [number, number, number] = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  const enc = (c: number) => {
    const v = Math.max(0, Math.min(1, c));
    return 255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
  };
  return [enc(lin[0]), enc(lin[1]), enc(lin[2])];
}

/** Mischt `b` mit Anteil `t` (0 … 1) in `a`, im OKLab-Raum. */
export function mix(a: string, b: string, t: number): string {
  const x = toOklab(parseHex(a));
  const y = toOklab(parseHex(b));
  return toHex(fromOklab([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]));
}

/** Schwarz oder Weiß, je nachdem, was auf `bg` besser lesbar ist (≥ 4,5:1 für jede Hintergrundfarbe). */
export function readableOn(bg: string): "#000000" | "#FFFFFF" {
  return contrast(bg, "#000000") >= contrast(bg, "#FFFFFF") ? "#000000" : "#FFFFFF";
}

/** Hex mit Deckkraft als `rgba(...)` (für Diff-Hintergründe). */
export function withAlpha(hex: string, alpha: number): string {
  const [r, g, b] = parseHex(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
