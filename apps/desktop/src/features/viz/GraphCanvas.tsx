// Zeichenfläche der Graphen und Bäume (UI-090, UI-146): Canvas mit Backing-Store = CSS-Größe × devicePixelRatio,
// Zoom per Rad/Pinch um den Mauszeiger (0,05× … 8×), Verschieben auf freier Fläche, Knoten ziehen (nur Graph),
// Tooltip nach 300 ms Verweilen. Zoom/Verschiebung laufen ohne React-Neuzeichnen direkt auf dem Canvas.
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState, type ReactNode } from "react";
import { drawScene, toSvg, type RenderStyle } from "./model/render";
import { HitIndex, type Scene } from "./model/scene";
import { toWorld, wheelFactor, zoomAt, type Transform } from "./model/viewport";

export const TOOLTIP_DELAY_MS = 300;
const DRAG_THRESHOLD = 3;
const PAN_STEP = 40;

export interface GraphCanvasHandle {
  getTransform(): Transform;
  setTransform(t: Transform): void;
  /** Startansicht neu berechnen (neue Wurzel). */
  resetView(): void;
  size(): { width: number; height: number };
  redraw(): void;
  svg(): string;
  /** PNG des aktuellen Ausschnitts in Faktor 1× oder 2× der Fenstergröße (UI-090). */
  png(factor: 1 | 2): Promise<Blob | null>;
}

export interface GraphCanvasProps {
  scene: Scene;
  /** Erhöhen, wenn sich die Szene geändert hat (Treffertest neu aufbauen, neu zeichnen). */
  version: number;
  style: RenderStyle;
  labelsAt: (k: number) => boolean;
  highlights: ReadonlySet<number>;
  current: number | null;
  draggable: boolean;
  label: string;
  initialTransform: (width: number, height: number) => Transform;
  onNodeClick?: (id: number) => void;
  onNodeDoubleClick?: (id: number) => void;
  onNodeDrag?: (id: number, x: number, y: number) => void;
  onNodeDragEnd?: (id: number) => void;
  onContextTarget?: (id: number | null) => void;
  onKeyTarget?: () => void;
  onFirstFrame?: () => void;
  tooltip?: (id: number) => ReactNode;
}

interface Press {
  x: number;
  y: number;
  node: number | null;
  dragging: boolean;
  start: Transform;
}

export const GraphCanvas = forwardRef<GraphCanvasHandle, GraphCanvasProps>(function GraphCanvas(props, ref) {
  const host = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const t = useRef<Transform | null>(null);
  const size = useRef({ width: 0, height: 0, dpr: 1 });
  const frame = useRef(0);
  const index = useRef<{ version: number; scene: Scene; hit: HitIndex } | null>(null);
  const press = useRef<Press | null>(null);
  const hoverTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const first = useRef(false);
  const live = useRef(props);
  live.current = props;
  const [hover, setHover] = useState<number | null>(null);
  const [tip, setTip] = useState<{ id: number; x: number; y: number } | null>(null);
  const hoverRef = useRef<number | null>(null);

  const transform = useCallback((): Transform => {
    if (t.current) return t.current;
    const init = live.current.initialTransform(size.current.width, size.current.height);
    // Erst mit bekannter Größe festhalten, sonst stimmt die Startansicht nicht.
    if (size.current.width > 0) t.current = init;
    return init;
  }, []);

  const state = useCallback(() => {
    const p = live.current;
    const tr = transform();
    return { t: tr, width: size.current.width, height: size.current.height, showLabels: p.labelsAt(tr.k), highlights: p.highlights, current: p.current, hovered: hoverRef.current };
  }, [transform]);

  const draw = useCallback(() => {
    frame.current = 0;
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx || size.current.width === 0) return;
    drawScene(ctx, live.current.scene, state(), live.current.style, size.current.dpr);
    if (!first.current && live.current.scene.nodes.length > 0) {
      first.current = true;
      live.current.onFirstFrame?.();
    }
  }, [state]);

  const schedule = useCallback(() => {
    if (frame.current) return;
    frame.current = typeof requestAnimationFrame === "function" ? requestAnimationFrame(draw) : (setTimeout(draw, 0) as unknown as number);
  }, [draw]);

  const hitIndex = useCallback((): HitIndex => {
    const p = live.current;
    if (!index.current || index.current.version !== p.version || index.current.scene !== p.scene) index.current = { version: p.version, scene: p.scene, hit: new HitIndex(p.scene) };
    return index.current.hit;
  }, []);

  const nodeAt = useCallback(
    (sx: number, sy: number): number | null => {
      const tr = transform();
      const [wx, wy] = toWorld(tr, sx, sy);
      return hitIndex().hit(wx, wy, 3 / tr.k)?.id ?? null;
    },
    [hitIndex, transform],
  );

  const setTransform = useCallback(
    (next: Transform) => {
      t.current = next;
      schedule();
    },
    [schedule],
  );

  useImperativeHandle(
    ref,
    () => ({
      getTransform: transform,
      setTransform,
      resetView: () => {
        t.current = null;
        schedule();
      },
      size: () => ({ width: size.current.width, height: size.current.height }),
      redraw: schedule,
      svg: () => toSvg(live.current.scene, state(), live.current.style),
      png: (factor) =>
        new Promise((resolve) => {
          const off = document.createElement("canvas");
          off.width = Math.max(1, Math.round(size.current.width * factor));
          off.height = Math.max(1, Math.round(size.current.height * factor));
          const ctx = off.getContext("2d");
          if (!ctx) return resolve(null);
          drawScene(ctx, live.current.scene, state(), live.current.style, factor);
          off.toBlob((b) => resolve(b), "image/png");
        }),
    }),
    [schedule, setTransform, state, transform],
  );

  // Größe und DPR nachführen: Backing-Store = CSS-Größe × devicePixelRatio (S-2/UI-120: scharf auf allen Zoomstufen).
  useEffect(() => {
    const el = host.current;
    const c = canvas.current;
    if (!el || !c) return;
    const resize = () => {
      const rect = el.getBoundingClientRect();
      const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
      const width = Math.max(0, rect.width);
      const height = Math.max(0, rect.height);
      size.current = { width, height, dpr };
      c.width = Math.max(1, Math.round(width * dpr));
      c.height = Math.max(1, Math.round(height * dpr));
      c.style.width = `${width}px`;
      c.style.height = `${height}px`;
      schedule();
    };
    resize();
    const ro = typeof ResizeObserver === "function" ? new ResizeObserver(resize) : null;
    ro?.observe(el);
    const media = typeof matchMedia === "function" ? matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`) : null;
    media?.addEventListener?.("change", resize);
    return () => {
      ro?.disconnect();
      media?.removeEventListener?.("change", resize);
    };
  }, [schedule]);

  useEffect(() => {
    schedule();
  }, [props.scene, props.version, props.style, props.highlights, props.current, hover, schedule]);

  useEffect(
    () => () => {
      // Kennung zurücksetzen: sonst plant schedule() nach einem erneuten Einhängen (StrictMode) nie wieder ein Bild.
      if (frame.current && typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame.current);
      frame.current = 0;
      if (hoverTimer.current) clearTimeout(hoverTimer.current);
    },
    [],
  );

  // Rad/Pinch: nicht passiv, damit die Seite nicht scrollt.
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = c.getBoundingClientRect();
      setTransform(zoomAt(transform(), wheelFactor(e.deltaY, e.deltaMode), e.clientX - rect.left, e.clientY - rect.top));
    };
    c.addEventListener("wheel", onWheel, { passive: false });
    return () => c.removeEventListener("wheel", onWheel);
  }, [setTransform, transform]);

  const local = (e: { clientX: number; clientY: number }): [number, number] => {
    const rect = canvas.current?.getBoundingClientRect();
    return [e.clientX - (rect?.left ?? 0), e.clientY - (rect?.top ?? 0)];
  };

  const clearTip = () => {
    if (hoverTimer.current) clearTimeout(hoverTimer.current);
    hoverTimer.current = null;
    setTip(null);
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    const [x, y] = local(e);
    canvas.current?.setPointerCapture?.(e.pointerId);
    press.current = { x, y, node: nodeAt(x, y), dragging: false, start: transform() };
    clearTip();
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const [x, y] = local(e);
    const p = press.current;
    if (p) {
      if (!p.dragging && Math.hypot(x - p.x, y - p.y) < DRAG_THRESHOLD) return;
      p.dragging = true;
      if (p.node !== null && live.current.draggable) {
        const [wx, wy] = toWorld(transform(), x, y);
        live.current.onNodeDrag?.(p.node, wx, wy);
        schedule();
      } else {
        setTransform({ k: p.start.k, x: p.start.x + (x - p.x), y: p.start.y + (y - p.y) });
      }
      return;
    }
    const id = nodeAt(x, y);
    if (id !== hoverRef.current) {
      hoverRef.current = id;
      setHover(id);
      clearTip();
      if (id !== null) hoverTimer.current = setTimeout(() => setTip({ id, x, y }), TOOLTIP_DELAY_MS);
    }
  };

  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const p = press.current;
    press.current = null;
    canvas.current?.releasePointerCapture?.(e.pointerId);
    if (!p) return;
    if (p.dragging) {
      if (p.node !== null && live.current.draggable) live.current.onNodeDragEnd?.(p.node);
      return;
    }
    if (p.node !== null) live.current.onNodeClick?.(p.node);
  };

  const onDoubleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const [x, y] = local(e);
    const id = nodeAt(x, y);
    if (id !== null) live.current.onNodeDoubleClick?.(id);
  };

  const onContextMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const [x, y] = local(e);
    clearTip();
    live.current.onContextTarget?.(nodeAt(x, y));
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    const tr = transform();
    const { width, height } = size.current;
    if (e.key === "+" || e.key === "=") setTransform(zoomAt(tr, 1.25, width / 2, height / 2));
    else if (e.key === "-") setTransform(zoomAt(tr, 0.8, width / 2, height / 2));
    else if (e.key === "ArrowLeft") setTransform({ ...tr, x: tr.x + PAN_STEP });
    else if (e.key === "ArrowRight") setTransform({ ...tr, x: tr.x - PAN_STEP });
    else if (e.key === "ArrowUp") setTransform({ ...tr, y: tr.y + PAN_STEP });
    else if (e.key === "ArrowDown") setTransform({ ...tr, y: tr.y - PAN_STEP });
    else if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      live.current.onKeyTarget?.();
      return;
    } else return;
    e.preventDefault();
  };

  return (
    <div ref={host} className="os-viz-stage" onPointerLeave={() => { hoverRef.current = null; setHover(null); clearTip(); }}>
      <canvas
        ref={canvas}
        className="os-viz-canvas"
        role="img"
        aria-label={props.label}
        tabIndex={0}
        data-hover={hover ?? undefined}
        style={{ cursor: hover !== null ? (props.draggable ? "grab" : "pointer") : "default" }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => (press.current = null)}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
        onKeyDown={onKeyDown}
      />
      {tip && props.tooltip && (
        <div className="os-viz-tooltip" role="tooltip" style={{ left: tip.x + 14, top: tip.y + 14 }}>
          {props.tooltip(tip.id)}
        </div>
      )}
    </div>
  );
});
