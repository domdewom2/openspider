// Zeichnen der Szene auf Canvas (UI-146: Canvas statt SVG-DOM) und SVG-Export des aktuellen Ausschnitts (UI-090:
// valides SVG 1.1, Texte als <text>, Farben als Hex). Beide nutzen dieselben Regeln für Sichtbarkeit und Beschriftung.
import { rgbToHex } from "./encoding";
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

/** Füllfarbe unter Berücksichtigung der Ausgrauung. */
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

export function drawScene(ctx: CanvasRenderingContext2D, scene: Scene, s: RenderState, style: RenderStyle, dpr: number): void {
  const { t } = s;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = style.background;
  ctx.fillRect(0, 0, s.width, s.height);
  ctx.setTransform(dpr * t.k, 0, 0, dpr * t.k, dpr * t.x, dpr * t.y);
  const visible = inView(scene, s);
  const shown = new Set(visible);

  // Kanten: ein Pfad für alle (sichtbar, wenn einer der Endpunkte im Ausschnitt liegt).
  ctx.beginPath();
  for (const [a, b] of scene.edges) {
    const na = scene.nodes[a] as SceneNode;
    const nb = scene.nodes[b] as SceneNode;
    if (!shown.has(na) && !shown.has(nb)) continue;
    ctx.moveTo(na.x, na.y);
    ctx.lineTo(nb.x, nb.y);
  }
  ctx.strokeStyle = s.overlay ? style.dim : style.edge;
  ctx.lineWidth = style.lineWidth / t.k;
  ctx.stroke();

  // Eingeblendete Links (UI-097): andere Farbe und Strichart als die Baumkanten.
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
      ctx.lineWidth = (style.lineWidth * 1.5) / t.k;
      ctx.stroke();
    }
    ctx.setLineDash([]);
  }

  // Gefüllte Knoten nach Farbe gebündelt.
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
    ctx.fillStyle = fill;
    ctx.fill();
  }
  // Hohle Pfadknoten und gestrichelte Ränder (fehlender Metrikwert).
  ctx.lineWidth = 1.5 / t.k;
  for (const n of visible) {
    if (!n.hollow && !n.dashed) continue;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
    ctx.setLineDash(n.dashed ? [3 / t.k, 2 / t.k] : []);
    ctx.strokeStyle = n.hollow && s.overlay && !s.overlay.connected.has(n.id) ? style.dim : (n.stroke ?? n.fill);
    ctx.stroke();
  }
  ctx.setLineDash([]);
  // Suchtreffer: Rahmen in Akzentfarbe; aktueller Treffer und Zeiger kräftiger.
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
  if (s.showLabels) {
    ctx.font = `${LABEL_PX / t.k}px ${style.fontFamily}`;
    ctx.textAlign = "left";
    ctx.fillStyle = style.text;
    for (const n of visible) ctx.fillText(n.label, n.x + n.r + 3 / t.k, n.y);
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
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
