// Knotenkodierung der Visualisierungen: Farben je Farbmodus, Radius nach Tiefe (UI-092) bzw. nach Metrik (UI-095).
// Alle Farben sind Hex-Werte, damit Canvas und SVG-Export (UI-090) dieselben Werte tragen.

export type ColorMode = "indexability" | "statusClass" | "segment" | "metric";
export type Palette = "standard" | "colorblind";

export interface PaletteColors {
  indexable: string;
  nonIndexable: string;
  capped: string;
  path: string;
  noData: string;
  status: { "2xx": string; "3xx": string; "4xx": string; "5xx": string; none: string };
  gradient: [string, string];
}

/** Standard: indexierbar grün, nicht indexierbar blassrot, gekappt grau (UI-091). Farbfehlsichtig: Blau/Orange-Paar. */
export const PALETTES: Record<Palette, PaletteColors> = {
  standard: {
    indexable: "#2f9e57",
    nonIndexable: "#e89a9a",
    capped: "#9ba1a8",
    path: "#7d8590",
    noData: "#c4c8cd",
    status: { "2xx": "#2f9e57", "3xx": "#3b7dd8", "4xx": "#e08a1e", "5xx": "#d13b3b", none: "#9ba1a8" },
    gradient: ["#d7e7f7", "#0b3c78"],
  },
  colorblind: {
    indexable: "#0072b2",
    nonIndexable: "#e69f00",
    capped: "#999999",
    path: "#666666",
    noData: "#cccccc",
    status: { "2xx": "#0072b2", "3xx": "#56b4e9", "4xx": "#e69f00", "5xx": "#d55e00", none: "#999999" },
    gradient: ["#fff3c4", "#7a3b00"],
  },
};

/** Parameter der Standard-Tiefenkodierung im kraftgerichteten Graphen (UI-092). */
export const DEPTH_DEFAULTS = { r0: 14, f: 0.8, minRadius: 3 } as const;
/** Fester Radius im Baum ohne Metrik-Skalierung. */
export const TREE_RADIUS = 5;
/** Standardgrenzen der Größenskalierung (UI-095, einstellbar). */
export const SIZE_DEFAULTS = { minRadius: 3, maxRadius: 30 } as const;

/** Radius nach Tiefe t: max(3, r₀ · fᵗ). */
export function depthRadius(t: number, r0: number = DEPTH_DEFAULTS.r0, f: number = DEPTH_DEFAULTS.f, minRadius: number = DEPTH_DEFAULTS.minRadius): number {
  return Math.max(minRadius, r0 * Math.pow(f, t));
}

/** Helligkeit (HSL-L, Prozent) nach Tiefe t: min(85 %, 30 % + 8 % · t). */
export function depthLightness(t: number): number {
  return Math.min(85, 30 + 8 * t);
}

/**
 * Größenskalierung: Fläche proportional zum Wert (Radius ∝ √Wert), größter Wert = Max-Radius, nie unter dem Min-Radius.
 * `null` (fehlender Wert) ergibt den Min-Radius; der Aufrufer zeichnet dann einen gestrichelten Rand.
 */
export function metricRadius(value: number | null | undefined, maxValue: number, minRadius: number, maxRadius: number): number {
  if (value == null || !Number.isFinite(value) || value <= 0 || !(maxValue > 0)) return minRadius;
  return Math.min(maxRadius, Math.max(minRadius, maxRadius * Math.sqrt(value / maxValue)));
}

export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const n = parseInt(full, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgbToHex(r: number, g: number, b: number): string {
  const c = (v: number) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

export function hexToHsl(hex: string): [number, number, number] {
  const [r, g, b] = hexToRgb(hex).map((v) => v / 255) as [number, number, number];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l * 100];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h *= 60;
  return [h, s * 100, l * 100];
}

export function hslToHex(h: number, s: number, l: number): string {
  const sat = s / 100;
  const lig = l / 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = sat * Math.min(lig, 1 - lig);
  const f = (n: number) => lig - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return rgbToHex(f(0) * 255, f(8) * 255, f(4) * 255);
}

/** Farbton der Basisfarbe beibehalten, Helligkeit setzen. */
export function withLightness(hex: string, lightness: number): string {
  const [h, s] = hexToHsl(hex);
  return hslToHex(h, s, lightness);
}

/** Linearer Verlauf zwischen zwei Hex-Farben, `t` in [0, 1]. */
export function mix(a: string, b: string, t: number): string {
  const x = hexToRgb(a);
  const y = hexToRgb(b);
  const u = Math.min(1, Math.max(0, t));
  return rgbToHex(x[0] + (y[0] - x[0]) * u, x[1] + (y[1] - x[1]) * u, x[2] + (y[2] - x[2]) * u);
}

export type StatusClass = keyof PaletteColors["status"];
export function statusClass(code: number | null | undefined): StatusClass {
  if (code == null || code < 200 || code >= 600) return "none";
  return `${Math.floor(code / 100)}xx` as StatusClass;
}

/** Normalisierte Segmentfarbe als Hex (Segmentfarben aus UI-068 kommen als Hex vom Kern). */
export function segmentHex(color: string | null | undefined, fallback: string): string {
  if (!color) return fallback;
  const c = color.trim();
  if (/^#[0-9a-f]{6}$/i.test(c)) return c.toLowerCase();
  if (/^#[0-9a-f]{3}$/i.test(c)) return rgbToHex(...hexToRgb(c));
  return fallback;
}

export interface EncodeNode {
  address?: string | null;
  indexability?: string | null;
  statusCode?: number | null;
  segmentColor?: string | null;
  metric?: number | null;
  colorValue?: number | null;
}

export interface EncodeOptions {
  layout: "tree" | "force";
  colorMode: ColorMode;
  palette: Palette;
  sizeByMetric: boolean;
  minRadius: number;
  maxRadius: number;
  r0: number;
  f: number;
}

export interface EncodeStats {
  maxMetric: number;
  minColor: number;
  maxColor: number;
}

export interface NodeStyle {
  r: number;
  fill: string;
  stroke: string | null;
  dashed: boolean;
  hollow: boolean;
}

export function isIndexable(n: EncodeNode): boolean {
  return n.indexability === "Indexable";
}

/** Werte für Metrik-Skalierung und Farbverlauf über die dargestellten Knoten. */
export function encodeStats(nodes: Iterable<EncodeNode>): EncodeStats {
  let maxMetric = 0;
  let minColor = Infinity;
  let maxColor = -Infinity;
  for (const n of nodes) {
    if (n.metric != null && Number.isFinite(n.metric) && n.metric > maxMetric) maxMetric = n.metric;
    if (n.colorValue != null && Number.isFinite(n.colorValue)) {
      if (n.colorValue < minColor) minColor = n.colorValue;
      if (n.colorValue > maxColor) maxColor = n.colorValue;
    }
  }
  return { maxMetric, minColor: Number.isFinite(minColor) ? minColor : 0, maxColor: Number.isFinite(maxColor) ? maxColor : 0 };
}

/**
 * Stil eines Knotens. `level` ist die Tiefe relativ zur Wurzel der Ansicht, `capped` = Kinder ausgeblendet (UI-096).
 * Metrik-Skalierung ersetzt die Tiefenkodierung vollständig (UI-092: keine Mischung).
 */
export function nodeStyle(n: EncodeNode, level: number, capped: boolean, o: EncodeOptions, s: EncodeStats): NodeStyle {
  const pal = PALETTES[o.palette];
  const path = n.address == null;
  const depthEncoded = o.layout === "force" && !o.sizeByMetric;
  let r: number;
  let dashed = false;
  if (o.sizeByMetric) {
    r = metricRadius(n.metric, s.maxMetric, o.minRadius, o.maxRadius);
    dashed = !path && (n.metric == null || !Number.isFinite(n.metric));
  } else if (depthEncoded) {
    r = depthRadius(level, o.r0, o.f, DEPTH_DEFAULTS.minRadius);
  } else {
    r = TREE_RADIUS;
  }
  if (path) return { r, fill: pal.path, stroke: pal.path, dashed: false, hollow: true };
  let fill: string;
  if (capped) fill = pal.capped;
  else if (o.colorMode === "indexability") {
    const base = isIndexable(n) ? pal.indexable : pal.nonIndexable;
    fill = depthEncoded ? withLightness(base, depthLightness(level)) : base;
  } else if (o.colorMode === "statusClass") fill = pal.status[statusClass(n.statusCode)];
  else if (o.colorMode === "segment") fill = segmentHex(n.segmentColor, pal.noData);
  else if (n.colorValue == null || !Number.isFinite(n.colorValue)) fill = pal.noData;
  else fill = mix(pal.gradient[0], pal.gradient[1], s.maxColor > s.minColor ? (n.colorValue - s.minColor) / (s.maxColor - s.minColor) : 1);
  return { r, fill, stroke: dashed ? "#5f6670" : null, dashed, hollow: false };
}
