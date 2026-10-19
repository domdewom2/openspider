// Ansichtstransformation der Zeichenfläche: Bildschirm = Welt · k + (x, y). Zoom hart begrenzt auf 0,05× … 8× (UI-090).

export interface Transform {
  k: number;
  x: number;
  y: number;
}

export const MIN_ZOOM = 0.05;
export const MAX_ZOOM = 8;
export const IDENTITY: Transform = { k: 1, x: 0, y: 0 };

export function clampZoom(k: number): number {
  if (!Number.isFinite(k)) return 1;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, k));
}

/** Zoomt um `factor`, der Weltpunkt unter (px, py) bleibt unter dem Mauszeiger. */
export function zoomAt(t: Transform, factor: number, px: number, py: number): Transform {
  const k = clampZoom(t.k * factor);
  const wx = (px - t.x) / t.k;
  const wy = (py - t.y) / t.k;
  return { k, x: px - wx * k, y: py - wy * k };
}

/** Mausrad/Pinch → Zoomfaktor (Pinch kommt als Rad mit Ctrl; Zeilenmodus skaliert stärker). */
export function wheelFactor(deltaY: number, deltaMode = 0): number {
  const d = deltaMode === 1 ? deltaY * 16 : deltaMode === 2 ? deltaY * 400 : deltaY;
  return Math.exp(-d * 0.002);
}

export function toWorld(t: Transform, sx: number, sy: number): [number, number] {
  return [(sx - t.x) / t.k, (sy - t.y) / t.k];
}

export function toScreen(t: Transform, wx: number, wy: number): [number, number] {
  return [wx * t.k + t.x, wy * t.k + t.y];
}

/** Gleiche Zoomstufe, Weltpunkt in die Mitte der Fläche w × h. */
export function centerOn(t: Transform, wx: number, wy: number, w: number, h: number): Transform {
  return { k: t.k, x: w / 2 - wx * t.k, y: h / 2 - wy * t.k };
}

export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/** Ganze Ausdehnung einpassen (Rand `pad` px), höchstens 1×. */
export function fitBounds(b: Bounds, w: number, h: number, pad = 24): Transform {
  const bw = Math.max(1, b.maxX - b.minX);
  const bh = Math.max(1, b.maxY - b.minY);
  const k = clampZoom(Math.min(1, (w - 2 * pad) / bw, (h - 2 * pad) / bh));
  return { k, x: (w - bw * k) / 2 - b.minX * k, y: (h - bh * k) / 2 - b.minY * k };
}

/** Sichtbarer Weltausschnitt (für Culling und SVG-Export des aktuellen Ausschnitts). */
export function worldView(t: Transform, w: number, h: number): Bounds {
  const [minX, minY] = toWorld(t, 0, 0);
  const [maxX, maxY] = toWorld(t, w, h);
  return { minX, minY, maxX, maxY };
}
