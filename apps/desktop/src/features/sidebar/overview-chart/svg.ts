// UI-102: geometry of the ring and the SVG 1.1 export (rules of UI-090: texts as <text>, colours as hex, hatches as
// <pattern>). The live chart uses the same geometry with token variables.
import { escapeXml } from "../../viz/model/render";

/** Slim ring: thickness 20 % of the outer radius; segments separated by a 2-px gap at mid radius. */
export const RING = { size: 160, outer: 72, inner: 57.6, gap: 2 } as const;
const C = RING.size / 2;
const FULL = Math.PI * 2;

const num = (v: number) => String(Math.round(v * 100) / 100);
function point(angle: number, radius: number): string {
  return `${num(C + radius * Math.sin(angle))} ${num(C - radius * Math.cos(angle))}`;
}

/** Annulus sector from `a0` to `a1` (radians, 0 = 12 o'clock, clockwise). A full turn is drawn as two halves. */
export function sectorPath(a0: number, a1: number, outer: number = RING.outer, inner: number = RING.inner): string {
  if (a1 - a0 >= FULL - 1e-9) {
    return `M ${point(0, outer)} A ${outer} ${outer} 0 1 1 ${point(Math.PI, outer)} A ${outer} ${outer} 0 1 1 ${point(0, outer)} Z `
      + `M ${point(0, inner)} A ${inner} ${inner} 0 1 0 ${point(Math.PI, inner)} A ${inner} ${inner} 0 1 0 ${point(0, inner)} Z`;
  }
  const large = a1 - a0 > Math.PI ? 1 : 0;
  return `M ${point(a0, outer)} A ${outer} ${outer} 0 ${large} 1 ${point(a1, outer)} L ${point(a1, inner)} A ${inner} ${inner} 0 ${large} 0 ${point(a0, inner)} Z`;
}

/**
 * Segments clockwise from 12 o'clock in the given order; each covers count / sum of the drawn counts (`a0`, `a1`).
 * The drawn path leaves a small gap to its neighbours (none for a single full ring); `mid` is the direction in which a
 * selected segment is lifted out.
 */
export function ringSegments(counts: readonly number[]): { d: string; a0: number; a1: number; mid: number }[] {
  const total = counts.reduce((s, c) => s + c, 0);
  const many = counts.filter(c => c > 0).length > 1;
  const pad = many ? RING.gap / ((RING.outer + RING.inner) / 2) : 0;
  let at = 0;
  return counts.map(count => {
    const a0 = at;
    at += total > 0 ? count / total * FULL : 0;
    const p = at - a0 > pad * 2 ? pad / 2 : 0;
    return { d: sectorPath(a0 + p, at - p), a0, a1: at, mid: (a0 + at) / 2 };
  });
}

/** Offset of a lifted segment along its middle direction. */
export function liftOffset(mid: number, distance = 3): { dx: number; dy: number } {
  return { dx: Math.round(Math.sin(mid) * distance * 100) / 100, dy: Math.round(-Math.cos(mid) * distance * 100) / 100 };
}

/** Hatch pattern of the n-th further category of one colour: diagonal, then the other diagonal, then vertical. */
export const HATCH_ANGLES = [45, -45, 0, 90] as const;
export function hatchAngle(hatch: number): number {
  return HATCH_ANGLES[(hatch - 1) % HATCH_ANGLES.length]!;
}

export interface SvgItem { label: string; count: string; pct: string; color: string; hatch: number; highlight?: boolean; value: number }
export interface SvgColors { surface: string; text: string; muted: string; focus: string; track: string }
export interface SvgInput {
  kind: "ring" | "bars";
  title: string;
  items: readonly SvgItem[];
  /** Legend lines without a drawn segment (0 or without counter). */
  tail: readonly { label: string; count: string; pct: string }[];
  center?: readonly [string, string];
  note?: string;
  colors: SvgColors;
  font: string;
  width: number;
  height: number;
}

const ROW = 18;
const PAD = 12;

function patternDefs(items: readonly SvgItem[], surface: string): { defs: string; fill: (item: SvgItem) => string } {
  const ids = new Map<string, string>();
  let defs = "";
  for (const item of items) {
    if (item.hatch <= 0) continue;
    const key = `${item.color}|${item.hatch}`;
    if (ids.has(key)) continue;
    const id = `hatch-${ids.size + 1}`;
    ids.set(key, id);
    defs += `<pattern id="${id}" patternUnits="userSpaceOnUse" width="6" height="6" patternTransform="rotate(${hatchAngle(item.hatch)})">`
      + `<rect x="0" y="0" width="6" height="6" fill="${item.color}"/><rect x="0" y="0" width="2" height="6" fill="${surface}"/></pattern>`;
  }
  return {
    defs: defs ? `<defs>${defs}</defs>` : "",
    fill: item => item.hatch > 0 ? `url(#${ids.get(`${item.color}|${item.hatch}`)})` : item.color,
  };
}

/** SVG 1.1 document of the chart (export). Sizes in CSS pixels of the displayed chart. */
export function chartSvg(input: SvgInput): string {
  const { colors, items } = input;
  const { defs, fill } = patternDefs(items, colors.surface);
  const text = (x: number, y: number, s: string, opts: { fill?: string; weight?: number; anchor?: string; size?: number; cls?: string } = {}) =>
    `<text${opts.cls ? ` class="${opts.cls}"` : ""} x="${num(x)}" y="${num(y)}" fill="${opts.fill ?? colors.text}" font-size="${opts.size ?? 12}"${opts.weight ? ` font-weight="${opts.weight}"` : ""}${opts.anchor ? ` text-anchor="${opts.anchor}"` : ""}>${escapeXml(s)}</text>`;
  const body: string[] = [];
  const width = Math.max(1, Math.round(input.width));
  body.push(text(PAD, PAD + 12, input.title, { weight: 600, size: 13, cls: "title" }));
  let y = PAD + 24;
  const legend = (x: number, top: number, maxWidth: number) => {
    let ly = top;
    for (const item of items) {
      body.push(`<rect x="${num(x)}" y="${num(ly + 3)}" width="10" height="10" rx="5" fill="${fill(item)}"${item.highlight ? ` stroke="${colors.focus}" stroke-width="2"` : ""}/>`);
      body.push(text(x + 16, ly + 12, `${item.label}`, { weight: item.highlight ? 700 : undefined, cls: "category" }));
      body.push(text(x + maxWidth, ly + 12, `${item.count} (${item.pct})`, { anchor: "end", cls: "value" }));
      ly += ROW;
    }
    for (const line of input.tail) {
      body.push(text(x + 16, ly + 12, line.label, { fill: colors.muted, cls: "category" }));
      body.push(text(x + maxWidth, ly + 12, line.pct ? `${line.count} (${line.pct})` : line.count, { fill: colors.muted, anchor: "end", cls: "value" }));
      ly += ROW;
    }
    return ly;
  };
  if (input.kind === "ring") {
    const segments = ringSegments(items.map(i => i.value));
    const side = width >= 2 * RING.size + 3 * PAD;
    body.push(`<g transform="translate(${PAD} ${y})">`);
    segments.forEach((s, i) => {
      const item = items[i]!;
      const lift = item.highlight ? liftOffset(s.mid) : null;
      body.push(`<path class="segment" d="${s.d}" fill="${fill(item)}" fill-rule="evenodd"${lift ? ` transform="translate(${lift.dx} ${lift.dy})"` : ""}/>`);
    });
    segments.forEach((s, i) => {
      if (!items[i]!.highlight) return;
      const lift = liftOffset(s.mid);
      body.push(`<path class="highlight" d="${s.d}" fill="none" fill-rule="evenodd" stroke="${colors.focus}" stroke-width="2" transform="translate(${lift.dx} ${lift.dy})"/>`);
    });
    if (input.center) {
      body.push(text(C, C + 6, input.center[0], { anchor: "middle", weight: 700, size: 24, cls: "center" }));
      body.push(text(C, C + 22, input.center[1], { anchor: "middle", fill: colors.muted, size: 11, cls: "center" }));
    }
    body.push("</g>");
    const legendEnd = side ? legend(2 * PAD + RING.size, y, width - 3 * PAD - RING.size) : legend(PAD, y + RING.size + PAD, width - 2 * PAD);
    y = Math.max(side ? y + RING.size : 0, legendEnd);
  } else {
    const max = Math.max(1, ...items.map(i => i.value));
    const labelWidth = Math.round((width - 2 * PAD) * 0.4);
    const valueWidth = 96;
    const barWidth = Math.max(8, width - 2 * PAD - labelWidth - valueWidth - 2 * 8);
    for (const item of items) {
      body.push(text(PAD, y + 12, item.label, { weight: item.highlight ? 700 : undefined, cls: "category" }));
      const w = Math.max(1, item.value / max * barWidth);
      body.push(`<rect x="${num(PAD + labelWidth + 8)}" y="${num(y + 4)}" width="${num(barWidth)}" height="10" rx="5" fill="${colors.track}"/>`);
      body.push(`<rect class="bar" x="${num(PAD + labelWidth + 8)}" y="${num(y + 4)}" width="${num(w)}" height="10" rx="5" fill="${fill(item)}"${item.highlight ? ` stroke="${colors.focus}" stroke-width="2"` : ""}/>`);
      body.push(text(width - PAD, y + 12, `${item.count} (${item.pct})`, { anchor: "end", cls: "value" }));
      y += ROW;
    }
    y = legend(PAD, y, width - 2 * PAD);
  }
  if (input.note) {
    body.push(text(PAD, y + 14, input.note, { fill: colors.muted, size: 11, cls: "note" }));
    y += ROW;
  }
  const height = Math.max(Math.round(input.height), Math.ceil(y + PAD));
  return `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" font-family="${escapeXml(input.font)}">`
    + `${defs}<rect x="0" y="0" width="${width}" height="${height}" fill="${colors.surface}"/>${body.join("")}</svg>`;
}

/** Pixel size of the PNG export: displayed size × factor × devicePixelRatio. */
export function pngSize(width: number, height: number, factor: 1 | 2, dpr: number): { width: number; height: number } {
  const scale = factor * (dpr > 0 ? dpr : 1);
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

/** Size attributes of an exported SVG. */
export function svgSize(svg: string): { width: number; height: number } {
  const m = /<svg[^>]*\swidth="(\d+)"[^>]*\sheight="(\d+)"/.exec(svg);
  return { width: Number(m?.[1] ?? 0), height: Number(m?.[2] ?? 0) };
}

/** Renders the SVG into a PNG (factor 1× or 2× of the displayed size, devicePixelRatio considered). */
export async function svgToPng(svg: string, factor: 1 | 2, dpr: number = globalThis.devicePixelRatio ?? 1): Promise<Blob> {
  const size = svgSize(svg);
  const target = pngSize(size.width, size.height, factor, dpr);
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const img = new Image();
    img.decoding = "async";
    await new Promise<void>((resolve, reject) => { img.onload = () => resolve(); img.onerror = () => reject(new Error("svg render")); img.src = url; });
    const canvas = document.createElement("canvas");
    canvas.width = target.width;
    canvas.height = target.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas");
    ctx.drawImage(img, 0, 0, target.width, target.height);
    return await new Promise<Blob>((resolve, reject) => canvas.toBlob(b => b ? resolve(b) : reject(new Error("png")), "image/png"));
  } finally {
    URL.revokeObjectURL(url);
  }
}
