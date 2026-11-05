// 3D-Varianten des Crawl- und Verzeichnisgraphen (UI-098): WebGL2-Szene mit allen Knoten in einem Zeichenaufruf (bis
// 100 000), Layout als 3D-Barnes-Hut im Worker (force3d.ts). Linksziehen dreht, Mausrad zoomt, Rechtsziehen verschiebt,
// Rechtsklick ohne Ziehen öffnet das Knoten-Kontextmenü (OS-Festlegung). Farbe nach Indexierbarkeit, Response Code,
// Segment oder Metrik, Suche fliegt zum Knoten, Links eines Knotens (UI-097), PNG-Export; ohne WebGL2 ein Hinweis.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getFormatters, useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import type { GraphDto } from "../../ipc/gen/GraphDto";
import { viz } from "../../ipc/viz";
import { Banner, ContextMenu, type MenuEntry } from "../../ui/primitives";
import { CompactLegend } from "./CompactLegend";
import { saveBlob, useRenderStyle } from "./exporters";
import * as cam from "./model/camera3d";
import { directoryPath, tooltipRows } from "./model/details";
import { encodeStats, nodeStyle, PALETTES } from "./model/encoding";
import type { ForceInput, ForceReply } from "./model/force";
import { createForceRunner, type ForceRunner } from "./model/forceRunner";
import { GraphGl, hasWebGL2, rgbOf, uploadScene, type GlColors, type RenderOptions3D } from "./model/gl3d";
import { placeLabels, plaque, type LabelBox } from "./model/render";
import { nearestK, viewDepth } from "./model/labels3d";
import { linkMenuEntries, overlayFrom, sameStructure, treeEdges, withLinks, type LinkOverlay, type LinkRequest } from "./model/links";
import { current as currentMatch, EMPTY_SEARCH, search as runSearch, step as stepSearch, type SearchState } from "./model/search";
import { VizTree } from "./model/tree";
import { wheelFactor } from "./model/viewport";
import { defaultOptions, encodeOptions, graphArgs, type VizOptions } from "./options";
import { isDirectoryKind, type VizWindowKind } from "./open";
import { Sidebar } from "./Sidebar";
import type { LayoutState } from "./GraphView";

export interface Graph3DViewProps {
  kind: VizWindowKind;
  snapshotId: string;
  generation: number;
  initialRoot: number | null;
}

export const TOOLTIP_DELAY_3D_MS = 300;
const DRAG_THRESHOLD = 3;
const MAX_LABELS = 150;
const FLY_MS = 750;
/** Kugelradius in Welteinheiten: Untergrenze und Obergrenze (≈ 3,5× – M3-V02 Optik). */
export const R3_MIN = 4;
export const R3_MAX = 14;
/** Grenzen der Punktgröße in CSS-Pixeln (Durchmesser) trotz Perspektive. */
const MIN_PX = 2;
const MAX_PX = 34;
/**
 * Durchmesser eines kleinsten Knotens (R3_MIN) in der eingepassten Gesamtansicht. Größen sind relativ zum
 * Einpass-Abstand: Zoomen vergrößert, entfernte Knoten werden kleiner (Perspektive), die Gesamtansicht bleibt lesbar.
 */
const LEAF_PX = 5;

/**
 * Kleinster Durchmesser je nach Knotenzahl: dichte Graphen (bis 100 000 Knoten) zeichnen kleinere Punkte – lesbarer und
 * ohne Leistungsrückschritt (Zahl der Bildpunkte je Bild bleibt etwa konstant, UI-098/UI-146).
 */
export function leafPx(nodes: number): number {
  return Math.max(MIN_PX, Math.min(LEAF_PX, LEAF_PX * Math.cbrt(3_000 / Math.max(1, nodes))));
}

/** Bildpunkte je Welteinheit im Abstand 1 (für Punktgröße und Treffertest). */
function pointScale(c: cam.Camera3D, pxPerCss: number, nodes: number): number {
  return ((leafPx(nodes) * pxPerCss) / (2 * R3_MIN)) * c.base;
}
/** Langsames Drehen im Leerlauf: Bogenmaß je Sekunde, Start nach dieser Ruhezeit. */
const AUTO_ROTATE_SPEED = 0.12;
const AUTO_ROTATE_IDLE_MS = 2_000;

/**
 * Radius einer Kugel: Tiefenkodierung (r₀ · fᵗ) bzw. Metrik-Skalierung (Fläche ∝ Wert) auf [R3_MIN, R3_MAX] abgebildet,
 * damit weder Riesen- noch Mikroknoten entstehen.
 */
export function radius3d(r: number, enc: { sizeByMetric: boolean; minRadius: number; maxRadius: number }): number {
  if (enc.sizeByMetric) {
    const span = Math.max(1e-6, enc.maxRadius - enc.minRadius);
    return R3_MIN + Math.min(1, Math.max(0, (r - enc.minRadius) / span)) * (R3_MAX - R3_MIN);
  }
  return Math.min(R3_MAX, Math.max(R3_MIN, r));
}

interface Loaded {
  tree: VizTree;
  dto: GraphDto;
}

/** Darstellbare 3D-Szene: Knoten in Breitensuche-Reihenfolge (Eltern vor Kindern, wie der Layout-Worker sie erwartet). */
interface Scene3D {
  ids: number[];
  index: Map<number, number>;
  parent: Int32Array;
  radius: Float32Array;
  fill: string[];
  edges: Uint32Array;
}

export interface Viz3DDebug {
  kind: VizWindowKind;
  webgl: boolean;
  nodes: number;
  edges: number;
  truncated: boolean;
  layout: LayoutState;
  camera: { k: number; theta: number; phi: number; target: [number, number, number] };
  matches: number[];
  links: { edges: number; total: number; connected: number[]; mode: string } | null;
  firstFrameMs: number | null;
  screenOf(id: number): [number, number] | null;
  positions(): { id: number; x: number; y: number; z: number }[];
  /** Zeichnet `ms` Millisekunden lang jedes Bild neu und liefert die Bildrate (Messrezept UI-098). */
  fps(ms: number): Promise<number>;
  setZoom(k: number): void;
}

declare global {
  interface Window {
    __osViz3d?: Viz3DDebug;
  }
}

function prefersReducedMotion(): boolean {
  try {
    return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function rgba(hex: string, alpha = 255): [number, number, number, number] {
  const [r, g, b] = rgbOf(hex);
  return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255), alpha];
}

export function Graph3DView({ kind, snapshotId, generation, initialRoot }: Graph3DViewProps) {
  const t = useT();
  const fmt = getFormatters();
  const supported = useMemo(() => hasWebGL2(), []);
  const [opts, setOpts] = useState<VizOptions>(() => defaultOptions(kind));
  const [data, setData] = useState<Loaded | null>(null);
  const [links, setLinks] = useState<LinkRequest | null>(null);
  const [overlay, setOverlay] = useState<LinkOverlay | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [layoutState, setLayoutState] = useState<LayoutState>({ phase: "idle", steps: 0 });
  const [searchState, setSearchState] = useState<SearchState>(EMPTY_SEARCH);
  const [menuTarget, setMenuTarget] = useState<number | null>(null);
  const menuTargetRef = useRef(menuTarget);
  menuTargetRef.current = menuTarget;
  const [tip, setTip] = useState<{ id: number; x: number; y: number } | null>(null);
  const [glFailed, setGlFailed] = useState(false);
  const style = useRenderStyle(opts.lineWidth);
  const host = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const labels = useRef<HTMLCanvasElement>(null);
  const gl = useRef<GraphGl | null>(null);
  const camera = useRef<cam.Camera3D>(cam.initialCamera(100));
  const fitted = useRef(false);
  /** Angezeigte Positionen (schwingen sanft auf `target` ein) und Positionen der Simulation. */
  const pos = useRef<Float32Array>(new Float32Array(0));
  const target = useRef<Float32Array>(new Float32Array(0));
  const easing = useRef(false);
  const sceneRadius = useRef(100);
  const lastInteraction = useRef(performance.now());
  const lastFrame = useRef(0);
  const haloIds = useRef<number[]>([]);
  /** Zählt Positionsänderungen (Layout, Einschwingen) für den Beschriftungs-Cache. */
  const posVersion = useRef(0);
  /** Letzte Zeigerposition; der Treffertest läuft höchstens einmal je Animationsbild. */
  const pointer = useRef<{ x: number; y: number } | null>(null);
  const pickFrame = useRef(0);
  const [contextLost, setContextLost] = useState(false);
  /** Kamera folgt dem wachsenden Layout (Einpassen), bis der Nutzer selbst dreht, zoomt, verschiebt oder sucht. */
  const autoFit = useRef(true);
  const runner = useRef<ForceRunner | null>(null);
  const size = useRef({ width: 0, height: 0, dpr: 1 });
  const frame = useRef(0);
  const continuous = useRef(0);
  const frames = useRef(0);
  const press = useRef<{ right: boolean; x: number; y: number; start: cam.Camera3D; moved: boolean } | null>(null);
  const synthetic = useRef(false);
  const hoverTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hovered = useRef<number | null>(null);
  const timing = useRef({ start: performance.now(), firstFrame: null as number | null });
  const loadSeq = useRef(0);
  const dataRef = useRef<Loaded | null>(null);
  dataRef.current = data;
  const rootId = initialRoot;

  // ---- Laden ----------------------------------------------------------------------------------------------------
  const linkKey = JSON.stringify(links);
  const argsKey = JSON.stringify(withLinks(graphArgs(kind, snapshotId, rootId, opts), links, opts));
  useEffect(() => {
    const seq = ++loadSeq.current;
    const ctrl = new AbortController();
    const req = JSON.parse(linkKey) as LinkRequest | null;
    setLoading(true);
    setError(null);
    viz
      .graph(JSON.parse(argsKey), ctrl.signal)
      .then((dto) => {
        if (seq !== loadSeq.current) return;
        const prev = dataRef.current;
        const tree = prev && sameStructure(prev.dto, dto) ? prev.tree : VizTree.from({ ...dto, edges: treeEdges(dto) });
        setOverlay(req ? overlayFrom(dto, req, tree) : null);
        setData({ tree, dto });
      })
      .catch((e: unknown) => {
        if (seq === loadSeq.current && !ctrl.signal.aborted) setError(describeError(t, toCmdError(e)).text);
      })
      .finally(() => seq === loadSeq.current && setLoading(false));
    return () => ctrl.abort();
  }, [argsKey, linkKey, generation, t]);

  const tree = data?.tree ?? null;
  const enc = useMemo(() => encodeOptions(kind, opts, style.palette), [kind, opts, style.palette]);

  // ---- Szene ------------------------------------------------------------------------------------------------------
  // Struktur (Knoten, Eltern, Kanten) getrennt vom Aussehen: ein Farbwechsel startet das Layout nicht neu.
  const structure = useMemo(() => {
    if (!tree) return null;
    const ids = tree.visible();
    const index = new Map(ids.map((id, i) => [id, i]));
    const parent = new Int32Array(ids.length);
    const edges: number[] = [];
    ids.forEach((id, i) => {
      const n = tree.get(id);
      const p = n?.parent != null ? (index.get(n.parent) ?? -1) : -1;
      parent[i] = p;
      if (p >= 0) edges.push(p, i);
    });
    return { ids, index, parent, edges: new Uint32Array(edges) };
  }, [tree]);
  const scene: Scene3D | null = useMemo(() => {
    if (!tree || !structure) return null;
    const stats = encodeStats(structure.ids.map((id) => tree.get(id)).filter((n) => n !== undefined));
    const radius = new Float32Array(structure.ids.length);
    const fill: string[] = [];
    structure.ids.forEach((id, i) => {
      const n = tree.get(id);
      const st = n ? nodeStyle(n, n.level, tree.isCapped(id), enc, stats) : null;
      radius[i] = radius3d(st?.r ?? 3, enc);
      fill.push(st ? (st.hollow ? PALETTES[enc.palette].path : st.fill) : "#999999");
    });
    return { ...structure, radius, fill };
  }, [structure, tree, enc]);
  const radiusKey = useMemo(() => (scene ? Array.from(scene.radius).join(",") : ""), [scene]);

  const highlights = useMemo(() => new Set(searchState.matches), [searchState.matches]);

  const colorArray = useMemo(() => {
    if (!scene) return null;
    const out = new Uint8Array(scene.ids.length * 4);
    const dim = rgba(style.dim, 110);
    const accent = rgba(style.accent);
    scene.ids.forEach((id, i) => {
      const c = highlights.has(id) ? accent : overlay && !overlay.connected.has(id) ? dim : rgba(scene.fill[i] as string);
      out.set(c, i * 4);
    });
    return out;
  }, [scene, overlay, highlights, style]);

  const overlayIdx = useMemo(() => {
    const inl: number[] = [];
    const outl: number[] = [];
    if (scene && overlay)
      for (const e of overlay.edges) {
        const a = scene.index.get(e.source);
        const b = scene.index.get(e.target);
        if (a === undefined || b === undefined) continue;
        (e.type === "inlink" ? inl : outl).push(a, b);
      }
    return { inlinks: new Uint32Array(inl), outlinks: new Uint32Array(outl), shown: (inl.length + outl.length) / 2 };
  }, [scene, overlay]);
  const colorArrayRef = useRef(colorArray);
  colorArrayRef.current = colorArray;
  const overlayIdxRef = useRef(overlayIdx);
  overlayIdxRef.current = overlayIdx;

  // ---- Zeichnen ---------------------------------------------------------------------------------------------------
  const glColors = useCallback((): GlColors => {
    const e = rgbOf(style.edge);
    const i = rgbOf(style.inlink);
    const o = rgbOf(style.outlink);
    const a = rgbOf(style.accent);
    // Viele eingeblendete Links werden transparenter, damit sie keine Fläche bilden.
    const la = Math.max(0.12, Math.min(0.9, 4 / Math.sqrt(Math.max(1, overlayIdx.shown))));
    return { backgroundCenter: rgbOf(style.background), backgroundEdge: rgbOf(style.backgroundEdge), edge: [e[0], e[1], e[2], style.dark ? 0.34 : 0.28], inlink: [i[0], i[1], i[2], la], outlink: [o[0], o[1], o[2], la], halo: [a[0], a[1], a[2], 0.55] };
  }, [style, overlayIdx.shown]);
  const colorsRef = useRef(glColors);
  colorsRef.current = glColors;
  const sceneRef = useRef(scene);
  sceneRef.current = scene;
  const overlayRef = useRef(overlay);
  overlayRef.current = overlay;
  const labelZoom = useRef(opts.labelZoom);
  labelZoom.current = opts.labelZoom;

  const radiusOfScene = useCallback(() => sceneRadius.current, []);
  const opt3d = useRef(opts);
  opt3d.current = opts;

  const matrices = useCallback((width: number, height: number) => {
    const c = camera.current;
    return { view: cam.viewMatrix(c), proj: cam.projMatrix(c, width / Math.max(1, height), Math.max(cam.distance(c), radiusOfScene())) };
  }, [radiusOfScene]);

  // Beschriftungen: Wurzel, Suchtreffer, Auswahl und Zeiger immer; ab Zoomstufe zusätzlich die nächsten Knoten. Ohne
  // Überlappung (Prioritätsreihenfolge), mit Plakette für die Lesbarkeit. Neu bestimmt nur, wenn sich Kamera,
  // Positionen, Größe, Hervorhebungen oder Einstellungen ändern (sonst bleibt die Beschriftungsfläche stehen); die
  // nächsten Knoten per Teilselektion über die Tiefe (`nearestK`), projiziert wird nur der Kandidatenkreis.
  const labelKey = useRef("");
  const drawLabels = (view: Float32Array, proj: Float32Array) => {
    const lc = labels.current;
    const ctx = lc?.getContext("2d");
    const sc = sceneRef.current;
    const tr = dataRef.current?.tree;
    if (!lc || !ctx || !sc || !tr) return;
    const { width, height, dpr } = size.current;
    const c = camera.current;
    const key = [c.target.join(","), c.theta, c.phi, c.k, c.base, posVersion.current, width, height, dpr, labelZoom.current, opts.labelMode, haloIds.current.join(","), overlayRef.current ? overlayRef.current.edges.length : -1, style.text, style.plaque, sc.ids.length].join("|");
    if (key === labelKey.current) return;
    labelKey.current = key;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const p = pos.current;
    const scaleCss = pointScale(c, 1, sc.ids.length);
    const at = (i: number) => cam.project(view, proj, [p[i * 3] as number, p[i * 3 + 1] as number, p[i * 3 + 2] as number], width, height);
    const inView = (q: { x: number; y: number } | null): q is { x: number; y: number; depth: number } => !!q && q.x >= 0 && q.y >= 0 && q.x <= width && q.y <= height;
    const strong = new Set<number>();
    const candidates: { i: number; x: number; y: number; r: number }[] = [];
    const push = (i: number | undefined, force: boolean) => {
      if (i === undefined) return;
      const s2 = at(i);
      if (!inView(s2)) return;
      const r = Math.min(MAX_PX, Math.max(MIN_PX, (2 * (sc.radius[i] as number) * scaleCss) / s2.depth)) / 2;
      if (force) strong.add(i);
      candidates.push({ i, x: s2.x, y: s2.y, r });
    };
    const prio = [...haloIds.current, tr.roots[0]].filter((x): x is number => x !== undefined && x !== null);
    for (const id of prio) push(sc.index.get(id), true);
    if (c.k >= labelZoom.current) {
      const ov = overlayRef.current;
      const near = nearestK(
        sc.ids.length,
        MAX_LABELS,
        (i) => (ov && !ov.connected.has(sc.ids[i] as number) ? -1 : viewDepth(view, p[i * 3] as number, p[i * 3 + 1] as number, p[i * 3 + 2] as number)),
        (i) => inView(at(i)),
      );
      for (const i of near) if (!strong.has(i)) push(i, false);
    }
    const text = (i: number) => {
      const n = tr.get(sc.ids[i] as number);
      return n ? (opts.labelMode === "url" ? (n.address ?? n.label) : n.label) : "";
    };
    const font = (b: boolean) => `${b ? 600 : 400} ${b ? 12 : 11}px ${style.fontFamily}`;
    const boxes: LabelBox[] = candidates.map((cd) => {
      ctx.font = font(strong.has(cd.i));
      const w = (ctx.measureText?.(text(cd.i)).width ?? 60) + 10;
      return { x: cd.x + cd.r + 4, y: cd.y - 9, w, h: 18 };
    });
    ctx.textBaseline = "middle";
    for (const k of placeLabels(boxes)) {
      const b = boxes[k] as LabelBox;
      const cd = candidates[k] as (typeof candidates)[number];
      ctx.globalAlpha = 0.9;
      plaque(ctx, b, 5, style.plaque, style.plaqueBorder, 1);
      ctx.globalAlpha = 1;
      ctx.font = font(strong.has(cd.i));
      ctx.fillStyle = style.text;
      ctx.fillText(text(cd.i), b.x + 5, b.y + b.h / 2);
    }
  };

  const renderOptions = (): RenderOptions3D => {
    const d = cam.distance(camera.current);
    const r = sceneRadius.current;
    const dpr = size.current.dpr;
    return { dimEdges: !!overlayRef.current, fogNear: Math.max(0.01, d - r * 0.25), fogFar: d + r * 1.15, minPx: MIN_PX * dpr, maxPx: MAX_PX * dpr };
  };

  // Zeichnen über eine Referenz: Effekte (WebGL-Kontext, Layout) hängen nicht an Stil- oder Optionswechseln.
  const drawRef = useRef<() => void>(() => undefined);
  drawRef.current = () => {
    frame.current = 0;
    const g = gl.current;
    const c = canvas.current;
    if (!g || !c || size.current.width === 0) return;
    const now = performance.now();
    const dt = lastFrame.current ? Math.min(100, now - lastFrame.current) : 16;
    lastFrame.current = now;
    // Sanftes Einschwingen: angezeigte Positionen folgen der Simulation.
    if (easing.current) {
      const p = pos.current;
      const q = target.current;
      let moved = 0;
      for (let i = 0; i < p.length; i++) {
        const d = (q[i] as number) - (p[i] as number);
        p[i] = (p[i] as number) + d * 0.28;
        if (Math.abs(d) > moved) moved = Math.abs(d);
      }
      if (moved < 0.05) {
        p.set(q);
        easing.current = false;
      }
      g.setPositions(p);
      posVersion.current++;
    }
    // Langsames Drehen im Leerlauf (Einstellung, Standard aus; nicht bei reduzierter Bewegung).
    const rotating = opt3d.current.autoRotate && !prefersReducedMotion() && !press.current && now - lastInteraction.current > AUTO_ROTATE_IDLE_MS;
    if (rotating) camera.current = { ...camera.current, theta: camera.current.theta + (AUTO_ROTATE_SPEED * dt) / 1000 };
    const { width, height, dpr } = size.current;
    const { view, proj } = matrices(width, height);
    const scale = pointScale(camera.current, dpr, sceneRef.current?.ids.length ?? 0);
    g.render(view, proj, c.width, c.height, scale, colorsRef.current(), renderOptions());
    drawLabels(view, proj);
    frames.current++;
    if (timing.current.firstFrame === null && (sceneRef.current?.ids.length ?? 0) > 0) {
      timing.current.firstFrame = performance.now() - timing.current.start;
      performance.mark?.("viz-first-frame");
    }
    if (easing.current || rotating || (opt3d.current.autoRotate && !prefersReducedMotion()) || continuous.current > now) frame.current = requestAnimationFrame(() => drawRef.current());
    else lastFrame.current = 0;
  };
  const draw = useCallback(() => drawRef.current(), []);

  const schedule = useCallback(() => {
    if (frame.current) return;
    frame.current = typeof requestAnimationFrame === "function" ? requestAnimationFrame(draw) : (setTimeout(draw, 0) as unknown as number);
  }, [draw]);

  const setCamera = useCallback((next: cam.Camera3D) => {
    camera.current = next;
    schedule();
  }, [schedule]);

  // WebGL-Kontext und Größe.
  useEffect(() => {
    const c = canvas.current;
    const el = host.current;
    if (!supported || !c || !el) return;
    const g = GraphGl.create(c);
    if (!g) {
      setGlFailed(true);
      return;
    }
    gl.current = g;
    let current: GraphGl | null = g;
    // Kontextverlust (GPU-Reset, Treiberwechsel, zu viele Kontexte): Wiederherstellung erlauben, Animation anhalten;
    // nach `webglcontextrestored` alles neu hochladen und zeichnen, sonst Hinweis statt leerer Fläche.
    const onLost = (e: Event) => {
      e.preventDefault();
      if (frame.current) cancelAnimationFrame(frame.current);
      frame.current = 0;
      cancelAnimationFrame(flight.current);
      gl.current = null;
      current = null;
      setContextLost(true);
    };
    const onRestored = () => {
      const next = GraphGl.create(c);
      if (!next) {
        setGlFailed(true);
        return;
      }
      current = next;
      gl.current = next;
      const sc = sceneRef.current;
      if (sc) {
        const o = overlayIdxRef.current;
        uploadScene(next, {
          radius: sc.radius,
          colors: colorArrayRef.current,
          edges: sc.edges,
          inlinks: o.inlinks,
          outlinks: o.outlinks,
          halo: new Uint32Array(haloIds.current.map((id) => sc.index.get(id)).filter((i): i is number => i !== undefined)),
          positions: pos.current,
        });
      }
      labelKey.current = "";
      setContextLost(false);
      draw();
    };
    c.addEventListener("webglcontextlost", onLost);
    c.addEventListener("webglcontextrestored", onRestored);
    const resize = () => {
      const rect = el.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      size.current = { width: rect.width, height: rect.height, dpr };
      for (const cv of [c, labels.current]) {
        if (!cv) continue;
        cv.width = Math.max(1, Math.round(rect.width * dpr));
        cv.height = Math.max(1, Math.round(rect.height * dpr));
        cv.style.width = `${rect.width}px`;
        cv.style.height = `${rect.height}px`;
      }
      draw();
    };
    resize();
    const ro = typeof ResizeObserver === "function" ? new ResizeObserver(resize) : null;
    ro?.observe(el);
    // Wechsel des Pixelverhältnisses (anderer Bildschirm, Zoom): neu skalieren und die Abfrage für den neuen Wert erneuern.
    let media: MediaQueryList | null = null;
    const watchDpr = () => {
      media?.removeEventListener?.("change", onDpr);
      media = typeof matchMedia === "function" ? matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`) : null;
      media?.addEventListener?.("change", onDpr);
    };
    const onDpr = () => {
      resize();
      watchDpr();
    };
    watchDpr();
    return () => {
      ro?.disconnect();
      media?.removeEventListener?.("change", onDpr);
      c.removeEventListener("webglcontextlost", onLost);
      c.removeEventListener("webglcontextrestored", onRestored);
      if (frame.current) cancelAnimationFrame(frame.current);
      frame.current = 0;
      cancelAnimationFrame(pickFrame.current);
      pickFrame.current = 0;
      current?.dispose();
      gl.current = null;
    };
  }, [supported, draw]);

  // Szene in die Puffer; Layout im Worker (3D-Barnes-Hut, deterministisch mit Startwert).
  useEffect(() => {
    const g = gl.current;
    const scene = sceneRef.current;
    if (!scene || !g) return;
    const n = scene.ids.length;
    g.setNodes(scene.radius, new Uint8Array(n * 4));
    g.setEdges(scene.edges);
    pos.current = new Float32Array(n * 3);
    target.current = new Float32Array(n * 3);
    easing.current = false;
    const input: ForceInput = {
      parent: scene.parent,
      radius: Float64Array.from(scene.radius),
      params: { repulsion: opts.repulsion, springLength: opts.springLength, seed: opts.seed, allowOverlap: true },
      dims: 3,
    };
    runner.current?.dispose();
    const startedAt = performance.now();
    const onReply = (r: ForceReply) => {
      const q = target.current;
      const count = q.length / 3;
      let cx = 0;
      let cy = 0;
      let cz = 0;
      for (let i = 0; i < q.length; i += 3) {
        q[i] = r.positions[i] as number;
        q[i + 1] = r.positions[i + 1] as number;
        q[i + 2] = r.positions[i + 2] as number;
        cx += q[i] as number;
        cy += q[i + 1] as number;
        cz += q[i + 2] as number;
      }
      const center: cam.Vec3 = count ? [cx / count, cy / count, cz / count] : [0, 0, 0];
      // Robuster Szenenradius: 95. Perzentil der Abstände zum Schwerpunkt (Stichprobe bei großen Graphen).
      const stride = Math.max(1, Math.floor(count / 4_000));
      const dist: number[] = [];
      for (let i = 0; i < count; i += stride) dist.push(Math.hypot((q[i * 3] as number) - center[0], (q[i * 3 + 1] as number) - center[1], (q[i * 3 + 2] as number) - center[2]));
      dist.sort((a, b) => a - b);
      const radius = Math.max(20, dist[Math.floor(dist.length * 0.95)] ?? 20);
      sceneRadius.current = Math.max(radius, dist[dist.length - 1] ?? radius);
      if (!fitted.current || prefersReducedMotion()) {
        pos.current.set(q);
        gl.current?.setPositions(pos.current);
        posVersion.current++;
      } else easing.current = true;
      if (!fitted.current || autoFit.current) {
        fitted.current = true;
        const c = camera.current;
        camera.current = { ...cam.initialCamera(radius * 1.08, center), theta: c.theta, phi: c.phi };
      }
      if (r.type === "done") {
        setLayoutState({ phase: "done", steps: r.steps, reason: r.reason, ms: performance.now() - startedAt });
      } else setLayoutState((s) => (s.phase === "paused" ? s : { phase: "running", steps: r.steps }));
      schedule();
    };
    const run = createForceRunner(onReply);
    runner.current = run;
    fitted.current = false;
    setLayoutState({ phase: "running", steps: 0 });
    run.start(input);
    return () => {
      run.dispose();
      if (runner.current === run) runner.current = null;
    };
    // radiusKey: neue Radien starten das Layout neu (wie im 2D-Graphen), Farben nicht.
  }, [structure, radiusKey, opts.repulsion, opts.springLength, opts.seed, radiusOfScene, schedule, glFailed]);

  useEffect(() => {
    if (!colorArray || !gl.current) return;
    gl.current.setColors(colorArray);
    schedule();
  }, [colorArray, radiusKey, schedule]);

  useEffect(() => {
    gl.current?.setOverlay(overlayIdx.inlinks, overlayIdx.outlinks);
    schedule();
  }, [overlayIdx, radiusKey, schedule]);

  // Lichtschein für Zeiger, aktuellen Suchtreffer und Knoten des offenen Kontextmenüs.
  const applyHalo = useCallback(() => {
    const sc = sceneRef.current;
    const ids = haloIds.current;
    const idx = ids.map((id) => sc?.index.get(id)).filter((i): i is number => i !== undefined);
    gl.current?.setHalo(new Uint32Array(idx));
    schedule();
  }, [schedule]);

  useEffect(() => schedule(), [style, opts.labelZoom, opts.labelMode, schedule]);

  // ---- Treffertest ------------------------------------------------------------------------------------------------
  const pick = useCallback((sx: number, sy: number): number | null => {
    const sc = sceneRef.current;
    if (!sc) return null;
    const { width, height } = size.current;
    const { view, proj } = matrices(width, height);
    const scale = pointScale(camera.current, 1, sceneRef.current?.ids.length ?? 0);
    const p = pos.current;
    let best = -1;
    let bestDepth = Infinity;
    for (let i = 0; i < sc.ids.length; i++) {
      const s = cam.project(view, proj, [p[i * 3] as number, p[i * 3 + 1] as number, p[i * 3 + 2] as number], width, height);
      if (!s) continue;
      const r = Math.min(MAX_PX / 2, Math.max(MIN_PX / 2, ((sc.radius[i] as number) * scale) / s.depth)) + 3;
      const dx = s.x - sx;
      const dy = s.y - sy;
      if (dx * dx + dy * dy <= r * r && s.depth < bestDepth) {
        best = i;
        bestDepth = s.depth;
      }
    }
    return best >= 0 ? (sc.ids[best] as number) : null;
  }, [matrices]);

  const screenOf = useCallback((id: number): [number, number] | null => {
    const sc = sceneRef.current;
    const i = sc?.index.get(id);
    if (i === undefined) return null;
    const { width, height } = size.current;
    const { view, proj } = matrices(width, height);
    const p = pos.current;
    const s = cam.project(view, proj, [p[i * 3] as number, p[i * 3 + 1] as number, p[i * 3 + 2] as number], width, height);
    return s ? [s.x, s.y] : null;
  }, [matrices]);

  // ---- Suche: fliegt zum Knoten -----------------------------------------------------------------------------------
  useEffect(() => {
    if (!tree) return;
    setSearchState((s) => (s.query ? runSearch(tree, s.query) : s));
  }, [tree]);
  const currentHit = currentMatch(searchState);
  const currentHitRef = useRef(currentHit);
  currentHitRef.current = currentHit;
  const flight = useRef(0);
  const flyTo = useCallback((id: number) => {
    const i = sceneRef.current?.index.get(id);
    if (i === undefined) return;
    const from = camera.current;
    const q = target.current;
    const to = cam.focusOn(from, [q[i * 3] as number, q[i * 3 + 1] as number, q[i * 3 + 2] as number]);
    lastInteraction.current = performance.now();
    autoFit.current = false;
    cancelAnimationFrame(flight.current);
    if (prefersReducedMotion()) return setCamera(to);
    const start = performance.now();
    const tick = () => {
      const u = (performance.now() - start) / FLY_MS;
      camera.current = cam.lerpCamera(from, to, u);
      draw();
      if (u < 1) flight.current = requestAnimationFrame(tick);
    };
    flight.current = requestAnimationFrame(tick);
  }, [draw, setCamera]);
  useEffect(() => {
    if (currentHit !== null) flyTo(currentHit);
  }, [currentHit, flyTo]);
  useEffect(() => {
    haloIds.current = [hovered.current, currentHit, menuTarget].filter((x): x is number => x !== null);
    applyHalo();
  }, [currentHit, menuTarget, applyHalo, radiusKey]);
  useEffect(() => {
    lastInteraction.current = performance.now();
    schedule();
  }, [opts.autoRotate, schedule]);

  // ---- Interaktion ------------------------------------------------------------------------------------------------
  const local = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = canvas.current?.getBoundingClientRect();
    return [e.clientX - (r?.left ?? 0), e.clientY - (r?.top ?? 0)];
  };
  const clearTip = () => {
    if (hoverTimer.current) clearTimeout(hoverTimer.current);
    hoverTimer.current = null;
    setTip(null);
  };
  const openMenuAt = (clientX: number, clientY: number, id: number | null) => {
    setMenuTarget(id);
    synthetic.current = true;
    canvas.current?.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX, clientY }));
  };

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      lastInteraction.current = performance.now();
    autoFit.current = false;
      const [x, y] = local(e);
      const { width, height } = size.current;
      setCamera(cam.zoomAt(camera.current, wheelFactor(e.deltaY, e.deltaMode), x - width / 2, y - height / 2, height));
    };
    c.addEventListener("wheel", onWheel, { passive: false });
    return () => c.removeEventListener("wheel", onWheel);
  }, [setCamera, supported, glFailed]);

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    lastInteraction.current = performance.now();
    autoFit.current = false;
    const right = e.button === 2 || (e.button === 0 && e.ctrlKey && /Mac/i.test(navigator.platform));
    if (e.button !== 0 && e.button !== 2) return;
    const [x, y] = local(e);
    canvas.current?.setPointerCapture?.(e.pointerId);
    press.current = { right, x, y, start: camera.current, moved: false };
    clearTip();
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const [x, y] = local(e);
    const p = press.current;
    if (p) {
      if (!p.moved && Math.hypot(x - p.x, y - p.y) < DRAG_THRESHOLD) return;
      p.moved = true;
      setCamera(p.right ? cam.pan(p.start, x - p.x, y - p.y, size.current.height) : cam.rotate(p.start, x - p.x, y - p.y));
      return;
    }
    // Treffertest höchstens einmal je Animationsbild mit der letzten Zeigerposition.
    pointer.current = { x, y };
    if (pickFrame.current) return;
    pickFrame.current = requestAnimationFrame(() => {
      pickFrame.current = 0;
      const at = pointer.current;
      if (!at || press.current) return;
      const id = pick(at.x, at.y);
      if (id !== hovered.current) {
        hovered.current = id;
        haloIds.current = [id, currentHitRef.current, menuTargetRef.current].filter((v): v is number => v !== null);
        applyHalo();
        clearTip();
        if (id !== null) hoverTimer.current = setTimeout(() => setTip({ id, x: at.x, y: at.y }), TOOLTIP_DELAY_3D_MS);
      }
    });
  };
  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const p = press.current;
    press.current = null;
    canvas.current?.releasePointerCapture?.(e.pointerId);
    if (!p || p.moved) return;
    if (p.right) {
      const [x, y] = local(e);
      openMenuAt(e.clientX, e.clientY, pick(x, y));
    }
  };
  // Natives Kontextmenü (macOS beim Drücken, Windows beim Loslassen) unterdrücken: Rechtsziehen verschiebt; das Menü
  // öffnet erst ein Rechtsklick ohne Ziehen (synthetisches Ereignis an den Kontextmenü-Rahmen).
  const onContextMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (synthetic.current) {
      synthetic.current = false;
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    lastInteraction.current = performance.now();
    autoFit.current = false;
    const c = camera.current;
    const { height } = size.current;
    let next: cam.Camera3D | null = null;
    if (e.key === "+" || e.key === "=") next = cam.zoomAt(c, 1.25, 0, 0, height);
    else if (e.key === "-") next = cam.zoomAt(c, 0.8, 0, 0, height);
    else if (e.key.startsWith("Arrow")) {
      const dx = e.key === "ArrowLeft" ? -40 : e.key === "ArrowRight" ? 40 : 0;
      const dy = e.key === "ArrowUp" ? -40 : e.key === "ArrowDown" ? 40 : 0;
      next = e.shiftKey ? cam.pan(c, -dx, -dy, height) : cam.rotate(c, dx, dy);
    } else if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      setMenuTarget(currentHit);
      return;
    }
    if (!next) return;
    e.preventDefault();
    setCamera(next);
  };

  // ---- Kontextmenü ------------------------------------------------------------------------------------------------
  const resetView = useCallback(() => {
    // Zurück zur eingepassten Gesamtansicht; die Kamera folgt dem Layout wieder.
    autoFit.current = true;
    const q = target.current;
    const n = q.length / 3 || 1;
    let cx = 0;
    let cy = 0;
    let cz = 0;
    for (let i = 0; i < q.length; i += 3) {
      cx += q[i] as number;
      cy += q[i + 1] as number;
      cz += q[i + 2] as number;
    }
    setCamera(cam.initialCamera(radiusOfScene() * 0.9, [cx / n, cy / n, cz / n]));
  }, [radiusOfScene, setCamera]);
  const menuEntries: MenuEntry[] = useMemo(() => {
    const fit: MenuEntry = { id: "viz-fit", label: t("viz-menu-reset-view"), onAction: resetView };
    const n = menuTarget !== null ? tree?.get(menuTarget) : undefined;
    if (!n) return [fit];
    return [
      {
        id: "viz-show-in-table",
        label: t("viz-menu-show-in-table"),
        isDisabled: n.address == null,
        disabledReason: n.address == null ? t("viz-reason-path-node") : undefined,
        onAction: () => void viz.focusInMain({ urlId: n.id }).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text)),
      },
      ...linkMenuEntries(t, n, overlay, setLinks, () => setLinks(null)),
      { ...fit, separatorBefore: true },
    ];
  }, [menuTarget, overlay, resetView, t, tree]);

  // ---- Export -----------------------------------------------------------------------------------------------------
  const exportPng = useCallback(async (factor: 1 | 2) => {
    const c = canvas.current;
    const g = gl.current;
    if (!c || !g) return;
    const { width, height } = size.current;
    const w = Math.max(1, Math.round(width * factor));
    const h = Math.max(1, Math.round(height * factor));
    const old = [c.width, c.height] as const;
    c.width = w;
    c.height = h;
    const { view, proj } = matrices(width, height);
    g.render(view, proj, w, h, pointScale(camera.current, factor, sceneRef.current?.ids.length ?? 0), colorsRef.current(), { ...renderOptions(), minPx: MIN_PX * factor, maxPx: MAX_PX * factor });
    // Bild = WebGL-Szene (mit Hintergrundverlauf) + Beschriftungen (wie auf dem Bildschirm).
    const out = document.createElement("canvas");
    out.width = w;
    out.height = h;
    const ctx = out.getContext("2d");
    if (ctx) {
      ctx.drawImage(c, 0, 0);
      if (labels.current) ctx.drawImage(labels.current, 0, 0, w, h);
    }
    const blob = await new Promise<Blob | null>((resolve) => (ctx ? out : c).toBlob(resolve, "image/png"));
    out.width = 0;
    out.height = 0;
    c.width = old[0];
    c.height = old[1];
    draw();
    if (blob) saveBlob(`${kind}@${factor}x.png`, blob);
  }, [draw, kind, matrices]);

  // ---- Testzugang -------------------------------------------------------------------------------------------------
  useEffect(() => {
    if (!import.meta.env.DEV && import.meta.env.VITE_E2E !== "1") return;
    const debug: Viz3DDebug = {
      kind,
      webgl: supported && !glFailed && !contextLost,
      nodes: scene?.ids.length ?? 0,
      edges: (scene?.edges.length ?? 0) / 2,
      truncated: data?.dto.truncated ?? false,
      layout: layoutState,
      get camera() {
        const c = camera.current;
        return { k: c.k, theta: c.theta, phi: c.phi, target: [...c.target] as [number, number, number] };
      },
      matches: searchState.matches,
      links: overlay ? { edges: overlayIdx.shown, total: overlay.total, connected: [...overlay.connected], mode: overlay.request.mode } : null,
      get firstFrameMs() {
        return timing.current.firstFrame;
      },
      screenOf,
      positions: () => scene?.ids.map((id, i) => ({ id, x: target.current[i * 3] as number, y: target.current[i * 3 + 1] as number, z: target.current[i * 3 + 2] as number })) ?? [],
      fps: (ms) =>
        new Promise((resolve) => {
          frames.current = 0;
          const start = performance.now();
          continuous.current = start + ms;
          schedule();
          setTimeout(() => resolve((frames.current * 1000) / Math.max(1, performance.now() - start)), ms + 50);
        }),
      setZoom: (k) => {
        autoFit.current = false;
        setCamera({ ...camera.current, k: Math.min(8, Math.max(0.05, k)) });
      },
    };
    window.__osViz3d = debug;
    // Gemeinsame Kennzahlen wie im 2D-Fenster (E2E-Hilfen warten auf `__osViz.nodes`).
    window.__osViz = { ...(window.__osViz ?? {}), kind, nodes: debug.nodes, edges: debug.edges, truncated: debug.truncated, layout: layoutState, matches: searchState.matches, links: debug.links } as never;
  });
  useEffect(() => () => {
    delete window.__osViz3d;
    delete window.__osViz;
  }, []);

  const tooltip = (id: number) => {
    const n = tree?.get(id);
    if (!n || !tree) return null;
    const rows = tooltipRows(n, directoryPath(tree, id), opts.metric, (v) => (Number.isInteger(v) ? fmt.integer(v) : fmt.decimal(v, 2)));
    return (
      <dl className="os-viz-tip">
        {rows.map((r, i) => (
          <div key={i} className="os-viz-tip-row">
            <dt>{r.name}</dt>
            <dd>{r.value}</dd>
          </div>
        ))}
      </dl>
    );
  };

  const noop = () => undefined;
  return (
    <div className="os-viz-body">
      <Sidebar
        kind={kind}
        options={opts}
        onOptions={(patch) => setOpts((o) => ({ ...o, ...patch }))}
        availableMetrics={data?.dto.availableMetrics ?? []}
        stats={null}
        scene={null}
        search={searchState}
        onSearch={(q) => setSearchState(tree ? runSearch(tree, q) : { ...EMPTY_SEARCH, query: q })}
        onStep={(dir) => setSearchState((s) => stepSearch(s, dir))}
        onExpandDepth={noop}
        layoutState={layoutState}
        onPause={() => {
          runner.current?.pause();
          setLayoutState((s) => ({ ...s, phase: "paused" }));
        }}
        onResume={() => {
          runner.current?.resume();
          setLayoutState((s) => ({ ...s, phase: "running" }));
        }}
        exportState={{ kind: "idle" }}
        onExportSvg={noop}
        onExportPng={(f) => void exportPng(f)}
        onExportCsv={noop}
        csvBlocked
        csvDir={null}
        onCsvDir={noop}
        hasData={supported && !glFailed && !!scene && scene.ids.length > 0}
        links={{ overlay, shown: overlayIdx.shown, onReset: () => setLinks(null), colors: { inlink: style.inlink, outlink: style.outlink } }}
      />
      <section className="os-viz-main" aria-label={t(`viz-kind-${kind}`)}>
        <nav className="os-viz-trail" aria-label={t("viz-trail-label")}>
          {loading && <span className="os-viz-loading" role="status">{t("viz-loading")}</span>}
          {data?.dto.truncated && <span className="os-viz-truncated">{t("viz-truncated", { count: scene?.ids.length ?? 0 })}</span>}
          <span className="os-viz-count" data-testid="viz-node-count">{t("viz-node-count", { count: scene?.ids.length ?? 0 })}</span>
        </nav>
        {error && (
          <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>
            {error}
          </Banner>
        )}
        {!supported || glFailed ? (
          <div className="os-viz-no-webgl" role="status" data-testid="viz-no-webgl">
            <p>{t("viz-3d-no-webgl")}</p>
          </div>
        ) : (
          <ContextMenu entries={menuEntries} label={t("viz-menu-label")} className="os-viz-stage-host">
            <div ref={host} className="os-viz-stage os-viz-stage-soft" onPointerLeave={() => { hovered.current = null; haloIds.current = [currentHit, menuTarget].filter((x): x is number => x !== null); applyHalo(); clearTip(); }}>
              <canvas
                ref={canvas}
                className="os-viz-canvas os-viz-canvas-3d"
                role="img"
                aria-label={t("viz-canvas-3d-label", { kind: t(`viz-kind-${kind}`), count: scene?.ids.length ?? 0 })}
                tabIndex={0}
                onPointerDown={onPointerDown}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={() => (press.current = null)}
                onContextMenu={onContextMenu}
                onKeyDown={onKeyDown}
              />
              <canvas ref={labels} className="os-viz-labels-3d" aria-hidden />
              {contextLost && (
                <p className="os-viz-context-lost" role="status" data-testid="viz-context-lost">
                  {t("viz-3d-context-lost")}
                </p>
              )}
              <CompactLegend
                colorMode={opts.colorMode}
                colors={enc.colors && opts.palette === "standard" ? enc.colors : PALETTES[opts.palette]}
                segments={opts.colorMode === "segment" && scene ? [...new Set(scene.fill)].filter((f) => f !== PALETTES[opts.palette].noData && f !== enc.colors?.noData) : []}
                directory={isDirectoryKind(kind)}
                links={overlay ? { inlink: overlay.edges.some((e) => e.type === "inlink"), outlink: overlay.edges.some((e) => e.type === "outlink"), inColor: style.inlink, outColor: style.outlink } : null}
              />
              {tip && (
                <div className="os-viz-tooltip" role="tooltip" style={{ left: tip.x + 14, top: tip.y + 14 }}>
                  {tooltip(tip.id)}
                </div>
              )}
            </div>
          </ContextMenu>
        )}
      </section>
    </div>
  );
}
