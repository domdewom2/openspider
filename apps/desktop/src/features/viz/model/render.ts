// Zeichnen der Szene auf Canvas (UI-146: Canvas statt SVG-DOM) und SVG-Export des aktuellen Ausschnitts (UI-090:
// valides SVG 1.1, Texte als <text>, Farben als Hex). Beide nutzen dieselben Regeln für Sichtbarkeit und Beschriftung.
import { rgbToHex, type PaletteColors } from "./encoding";
import type { Scene, SceneNode, SceneOverlay } from "./scene";
import { worldView, type Transform } from "./viewport";

export interface RenderStyle {
  background: string;
  edge: string;
  text: string;
  accent: string;
  badgeFill: string;
  badgeText: string;
  /** Linienbreite der Kanten in Bildschirm-px. */
  lineWidth: number;
  fontFamily: string;
  /** Eingeblendete Inlinks/Outlinks (UI-097) und ausgegraute, nicht verbundene Knoten. */
  inlink: string;
  outlink: string;
  dim: string;
  /** Rand des Hintergrundverlaufs (Mitte = `background`), Plakette hinter Beschriftungen, dunkles Theme. */
  backgroundEdge: string;
  plaque: string;
  plaqueBorder: string;
  dark: boolean;
  /** Standardschema aus Tokens (siehe `paletteFromTokens`). */
  palette: PaletteColors | null;
}

export const DEFAULT_STYLE: RenderStyle = {
  background: "#ffffff",
  edge: "#b8bec6",
  text: "#1f2328",
  accent: "#0b6bcb",
  badgeFill: "#5f6670",
  badgeText: "#ffffff",
  lineWidth: 1,
  fontFamily: "sans-serif",
  inlink: "#0b6bcb",
  outlink: "#c25e00",
  dim: "#d5d9de",
  backgroundEdge: "#eef0f3",
  plaque: "#ffffff",
  plaqueBorder: "#d5d9df",
  dark: false,
  palette: null,
};

export interface RenderState {
  t: Transform;
  width: number;
  height: number;
  /** Beschriftungen zeigen (Baum ab Zoom 0,6; Graph ab eingestellter Zoomstufe, UI-092). */
  showLabels: boolean;
  highlights: ReadonlySet<number>;
  current: number | null;
  hovered: number | null;
  /** Eingeblendete Links; verbundene Knoten behalten ihre Farbe, alle übrigen werden ausgegraut (UI-097). */
  overlay?: SceneOverlay | null;
}

/** Füllfarbe unter Berücksichtigung der Ausgrauung (deutlich zurückgenommen). */
function fillOf(n: SceneNode, s: RenderState, style: RenderStyle): string {
  return s.overlay && !s.overlay.connected.has(n.id) ? style.dim : n.fill;
}

const LABEL_PX = 11;
const BADGE_PX = 9;

/** Knoten, die den Ausschnitt berühren (Culling). */
export function inView(scene: Scene, s: Pick<RenderState, "t" | "width" | "height">): SceneNode[] {
  const v = worldView(s.t, s.width, s.height);
  const pad = 40 / s.t.k;
  return scene.nodes.filter((n) => n.x + n.r >= v.minX - pad && n.x - n.r <= v.maxX + pad && n.y + n.r >= v.minY - pad && n.y - n.r <= v.maxY + pad);
}

function badgeText(n: number): string {
  return n > 9999 ? `${Math.floor(n / 1000)}k` : String(n);
}

/** Rechteck in Bildschirmkoordinaten (Kollisionsprüfung der Beschriftungen). */
export interface LabelBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Beschriftungen ohne Überlappung (M3-V02, Optik): in Prioritätsreihenfolge platzieren, eine Beschriftung, die eine
 * bereits platzierte Plakette schneidet, entfällt. Liefert die Indizes der gezeigten Beschriftungen.
 */
export function placeLabels(boxes: readonly LabelBox[], gap = 2): number[] {
  const placed: LabelBox[] = [];
  const out: number[] = [];
  // Grobes Raster beschleunigt die Suche bei vielen Kandidaten.
  const cell = 64;
  const grid = new Map<string, number[]>();
  const keys = (b: LabelBox) => {
    const k: string[] = [];
    for (let gx = Math.floor(b.x / cell); gx <= Math.floor((b.x + b.w) / cell); gx++)
      for (let gy = Math.floor(b.y / cell); gy <= Math.floor((b.y + b.h) / cell); gy++) k.push(`${gx}:${gy}`);
    return k;
  };
  boxes.forEach((b, i) => {
    const ks = keys(b);
    for (const k of ks)
      for (const j of grid.get(k) ?? []) {
        const o = placed[j] as LabelBox;
        if (b.x < o.x + o.w + gap && b.x + b.w + gap > o.x && b.y < o.y + o.h + gap && b.y + b.h + gap > o.y) return;
      }
    const id = placed.push(b) - 1;
    for (const k of ks) {
      const list = grid.get(k);
      if (list) list.push(id);
      else grid.set(k, [id]);
    }
    out.push(i);
  });
  return out;
}

/** Abgerundete Plakette hinter einer Beschriftung. */
export function plaque(ctx: CanvasRenderingContext2D, b: LabelBox, r: number, fill: string, stroke: string, lineWidth: number): void {
  ctx.beginPath();
  ctx.moveTo(b.x + r, b.y);
  ctx.arcTo(b.x + b.w, b.y, b.x + b.w, b.y + b.h, r);
  ctx.arcTo(b.x + b.w, b.y + b.h, b.x, b.y + b.h, r);
  ctx.arcTo(b.x, b.y + b.h, b.x, b.y, r);
  ctx.arcTo(b.x, b.y, b.x + b.w, b.y, r);
  ctx.closePath();
  ctx.fillStyle = fill;
  ctx.fill();
  ctx.lineWidth = lineWidth;
  ctx.strokeStyle = stroke;
  ctx.stroke();
}

/** Hintergrund: weicher radialer Verlauf von der Mitte (`background`) zum Rand (`backgroundEdge`). */
export function paintBackground(ctx: CanvasRenderingContext2D, width: number, height: number, style: RenderStyle): void {
  const g = ctx.createRadialGradient?.(width * 0.5, height * 0.42, 0, width * 0.5, height * 0.5, Math.max(width, height) * 0.75);
  if (g) {
    g.addColorStop(0, style.background);
    g.addColorStop(1, style.backgroundEdge);
    ctx.fillStyle = g;
  } else ctx.fillStyle = style.background;
  ctx.fillRect(0, 0, width, height);
}

export function drawScene(ctx: CanvasRenderingContext2D, scene: Scene, s: RenderState, style: RenderStyle, dpr: number): void {
  const { t } = s;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  paintBackground(ctx, s.width, s.height, style);
  ctx.setTransform(dpr * t.k, 0, 0, dpr * t.k, dpr * t.x, dpr * t.y);
  const visible = inView(scene, s);
  const shown = new Set(visible);
  const dimmed = (n: SceneNode) => !!s.overlay && !s.overlay.connected.has(n.id);

  // Baumkanten zurückhaltend (halbtransparent), bei eingeblendeten Links noch weiter zurückgenommen.
  ctx.beginPath();
  for (const [a, b] of scene.edges) {
    const na = scene.nodes[a] as SceneNode;
    const nb = scene.nodes[b] as SceneNode;
    if (!shown.has(na) && !shown.has(nb)) continue;
    ctx.moveTo(na.x, na.y);
    ctx.lineTo(nb.x, nb.y);
  }
  ctx.globalAlpha = s.overlay ? 0.3 : 0.5;
  ctx.strokeStyle = style.edge;
  ctx.lineWidth = (style.lineWidth * 0.75) / t.k;
  ctx.stroke();
  ctx.globalAlpha = 1;

  // Eingeblendete Links (UI-097): andere Farbe und Strichart als die Baumkanten, kräftiger.
  if (s.overlay) {
    for (const type of ["inlink", "outlink"] as const) {
      ctx.beginPath();
      for (const [a, b, kind] of s.overlay.edges) {
        if (kind !== type) continue;
        const na = scene.nodes[a] as SceneNode;
        const nb = scene.nodes[b] as SceneNode;
        if (!shown.has(na) && !shown.has(nb)) continue;
        ctx.moveTo(na.x, na.y);
        ctx.lineTo(nb.x, nb.y);
      }
      ctx.setLineDash(type === "outlink" ? [6 / t.k, 3 / t.k] : []);
      ctx.strokeStyle = type === "inlink" ? style.inlink : style.outlink;
      // Viele Links werden transparenter, damit sie keine Fläche bilden.
      ctx.globalAlpha = Math.max(0.12, Math.min(0.85, 4 / Math.sqrt(Math.max(1, s.overlay.edges.length))));
      ctx.lineWidth = (style.lineWidth * (s.overlay.edges.length > 60 ? 1 : 1.4)) / t.k;
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;
  }

  // Gefüllte Knoten nach Farbe gebündelt, mit feinem Rand in Hintergrundfarbe (trennt dicht liegende Knoten).
  const byFill = new Map<string, SceneNode[]>();
  for (const n of visible) {
    if (n.hollow) continue;
    const fill = fillOf(n, s, style);
    const list = byFill.get(fill);
    if (list) list.push(n);
    else byFill.set(fill, [n]);
  }
  for (const [fill, list] of byFill) {
    ctx.beginPath();
    for (const n of list) {
      ctx.moveTo(n.x + n.r, n.y);
      ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
    }
    ctx.globalAlpha = list.length && dimmed(list[0] as SceneNode) ? 0.6 : 1;
    ctx.fillStyle = fill;
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.lineWidth = 1 / t.k;
    ctx.strokeStyle = style.background;
    ctx.stroke();
  }
  // Hohle Pfadknoten und gestrichelte Ränder (fehlender Metrikwert).
  ctx.lineWidth = 1.5 / t.k;
  for (const n of visible) {
    if (!n.hollow && !n.dashed) continue;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
    ctx.setLineDash(n.dashed ? [3 / t.k, 2 / t.k] : []);
    ctx.strokeStyle = n.hollow && dimmed(n) ? style.dim : (n.stroke ?? n.fill);
    ctx.stroke();
  }
  ctx.setLineDash([]);
  // Zeiger: weicher Schein; Suchtreffer: Rahmen in Akzentfarbe, aktueller Treffer kräftiger.
  for (const n of visible) {
    if (n.id !== s.hovered) continue;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.r + 5 / t.k, 0, Math.PI * 2);
    ctx.fillStyle = style.accent;
    ctx.globalAlpha = 0.18;
    ctx.fill();
    ctx.globalAlpha = 1;
  }
  for (const n of visible) {
    const hit = s.highlights.has(n.id);
    if (!hit && n.id !== s.hovered) continue;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.r + 2.5 / t.k, 0, Math.PI * 2);
    ctx.lineWidth = (n.id === s.current ? 3 : 2) / t.k;
    ctx.strokeStyle = hit ? style.accent : style.text;
    ctx.stroke();
  }
  // Badges gekappter Knoten (Anzahl ausgeblendeter Kinder, UI-096).
  ctx.font = `${BADGE_PX / t.k}px ${style.fontFamily}`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  for (const n of visible) {
    if (!n.badge) continue;
    const bx = n.x + n.r * 0.85;
    const by = n.y - n.r * 0.85;
    ctx.beginPath();
    ctx.arc(bx, by, 7 / t.k, 0, Math.PI * 2);
    ctx.fillStyle = style.badgeFill;
    ctx.fill();
    ctx.fillStyle = style.badgeText;
    ctx.fillText(badgeText(n.badge), bx, by);
  }
  if (s.showLabels) drawLabels(ctx, visible, s, style, dpr);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

/** Beschriftungen in Bildschirmkoordinaten mit Plakette, ohne Überlappung; Treffer und Zeiger zuerst, dann große Knoten. */
function drawLabels(ctx: CanvasRenderingContext2D, visible: readonly SceneNode[], s: RenderState, style: RenderStyle, dpr: number): void {
  const { t } = s;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.font = `${LABEL_PX}px ${style.fontFamily}`;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  const rank = (n: SceneNode) => (n.id === s.current ? 0 : n.id === s.hovered ? 1 : s.highlights.has(n.id) ? 2 : 3);
  const order = [...visible].filter((n) => !(s.overlay && !s.overlay.connected.has(n.id)) || rank(n) < 3).sort((a, b) => rank(a) - rank(b) || b.r - a.r);
  const boxes = order.map((n) => {
    const w = (ctx.measureText?.(n.label).width ?? n.label.length * 6) + 8;
    return { x: n.x * t.k + t.x + n.r * t.k + 3, y: n.y * t.k + t.y - 8, w, h: 16 };
  });
  for (const i of placeLabels(boxes)) {
    const b = boxes[i] as LabelBox;
    const n = order[i] as SceneNode;
    ctx.globalAlpha = 0.88;
    plaque(ctx, b, 4, style.plaque, style.plaqueBorder, 1);
    ctx.globalAlpha = 1;
    ctx.fillStyle = style.text;
    ctx.fillText(n.label, b.x + 4, b.y + b.h / 2);
  }
}

/** Beliebige CSS-Farbe (Hex, rgb()) als Hex; sonst Rückfall. */
export function cssToHex(color: string | null | undefined, fallback: string): string {
  const c = (color ?? "").trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(c)) return c;
  if (/^#[0-9a-f]{3}$/.test(c)) return `#${c[1]}${c[1]}${c[2]}${c[2]}${c[3]}${c[3]}`;
  const m = /^rgba?\(\s*([\d.]+)[ ,]+([\d.]+)[ ,]+([\d.]+)/.exec(c);
  if (m) return rgbToHex(Number(m[1]), Number(m[2]), Number(m[3]));
  return fallback;
}

export function escapeXml(text: string): string {
  return text.replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" })[c] as string);
}

const num = (v: number) => String(Math.round(v * 100) / 100);

/**
 * SVG 1.1 des aktuellen Ausschnitts: Knotenkreise in Weltkoordinaten (Radius unverändert prüfbar, UI-092), Ansicht als
 * Matrix der Gruppe. Knotenbeschriftungen nur bei `showLabels` (Klasse `label`), Badges mit Klasse `badge`.
 */
export function toSvg(scene: Scene, s: RenderState, style: RenderStyle): string {
  const { t } = s;
  const visible = inView(scene, s);
  const shown = new Set(visible);
  const out: string[] = [];
  const w = Math.round(s.width);
  const h = Math.round(s.height);
  const fg = cssToHex(style.text, DEFAULT_STYLE.text);
  out.push('<?xml version="1.0" encoding="UTF-8"?>');
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">`);
  out.push(`<rect x="0" y="0" width="${w}" height="${h}" fill="${cssToHex(style.background, DEFAULT_STYLE.background)}"/>`);
  out.push(`<g transform="matrix(${num(t.k)} 0 0 ${num(t.k)} ${num(t.x)} ${num(t.y)})">`);
  const d: string[] = [];
  for (const [a, b] of scene.edges) {
    const na = scene.nodes[a] as SceneNode;
    const nb = scene.nodes[b] as SceneNode;
    if (!shown.has(na) && !shown.has(nb)) continue;
    d.push(`M${num(na.x)} ${num(na.y)}L${num(nb.x)} ${num(nb.y)}`);
  }
  if (d.length) out.push(`<path class="edges" fill="none" stroke="${cssToHex(s.overlay ? style.dim : style.edge, DEFAULT_STYLE.edge)}" stroke-width="${num(style.lineWidth / t.k)}" d="${d.join("")}"/>`);
  if (s.overlay) {
    for (const type of ["inlink", "outlink"] as const) {
      const ld: string[] = [];
      for (const [a, b, kind] of s.overlay.edges) {
        if (kind !== type) continue;
        const na = scene.nodes[a] as SceneNode;
        const nb = scene.nodes[b] as SceneNode;
        if (!shown.has(na) && !shown.has(nb)) continue;
        ld.push(`M${num(na.x)} ${num(na.y)}L${num(nb.x)} ${num(nb.y)}`);
      }
      if (!ld.length) continue;
      const color = cssToHex(type === "inlink" ? style.inlink : style.outlink, type === "inlink" ? DEFAULT_STYLE.inlink : DEFAULT_STYLE.outlink);
      const dash = type === "outlink" ? ` stroke-dasharray="${num(6 / t.k)} ${num(3 / t.k)}"` : "";
      out.push(`<path class="edges-${type}" fill="none" stroke="${color}" stroke-width="${num((style.lineWidth * 1.5) / t.k)}"${dash} d="${ld.join("")}"/>`);
    }
  }
  const dim = cssToHex(style.dim, DEFAULT_STYLE.dim);
  const strokeW = num(1.5 / t.k);
  for (const n of visible) {
    // Kennung als XML-Name: Pfadknoten haben negative Kennungen (n-5 → nm5).
    const dimmed = !!s.overlay && !s.overlay.connected.has(n.id);
    const fill = dimmed ? dim : n.fill;
    const cls = ["node", ...(s.highlights.has(n.id) ? ["hit"] : []), ...(dimmed ? ["dim"] : [])].join(" ");
    const attrs = [`id="n${n.id < 0 ? `m${-n.id}` : n.id}"`, `class="${cls}"`, `cx="${num(n.x)}"`, `cy="${num(n.y)}"`, `r="${num(n.r)}"`];
    if (n.hollow) attrs.push(`fill="none"`, `stroke="${dimmed ? dim : (n.stroke ?? n.fill)}"`, `stroke-width="${strokeW}"`);
    else if (n.dashed) attrs.push(`fill="${fill}"`, `stroke="${n.stroke ?? fg}"`, `stroke-width="${strokeW}"`, `stroke-dasharray="${num(3 / t.k)} ${num(2 / t.k)}"`);
    else attrs.push(`fill="${fill}"`);
    out.push(`<circle ${attrs.join(" ")}/>`);
  }
  const accent = cssToHex(style.accent, DEFAULT_STYLE.accent);
  for (const n of visible) {
    if (!s.highlights.has(n.id)) continue;
    out.push(`<circle class="hit-ring" cx="${num(n.x)}" cy="${num(n.y)}" r="${num(n.r + 2.5 / t.k)}" fill="none" stroke="${accent}" stroke-width="${num(2 / t.k)}"/>`);
  }
  const badgeFill = cssToHex(style.badgeFill, DEFAULT_STYLE.badgeFill);
  const badgeColor = cssToHex(style.badgeText, DEFAULT_STYLE.badgeText);
  for (const n of visible) {
    if (!n.badge) continue;
    const bx = num(n.x + n.r * 0.85);
    const by = num(n.y - n.r * 0.85);
    out.push(`<circle class="badge" cx="${bx}" cy="${by}" r="${num(7 / t.k)}" fill="${badgeFill}"/>`);
    out.push(`<text class="badge" x="${bx}" y="${by}" dy="0.35em" text-anchor="middle" font-family="${escapeXml(style.fontFamily)}" font-size="${num(BADGE_PX / t.k)}" fill="${badgeColor}">${badgeText(n.badge)}</text>`);
  }
  if (s.showLabels) {
    for (const n of visible) {
      out.push(`<text class="label" x="${num(n.x + n.r + 3 / t.k)}" y="${num(n.y)}" dy="0.35em" font-family="${escapeXml(style.fontFamily)}" font-size="${num(LABEL_PX / t.k)}" fill="${fg}">${escapeXml(n.label)}</text>`);
    }
  }
  out.push("</g>");
  out.push("</svg>");
  return out.join("\n");
}
