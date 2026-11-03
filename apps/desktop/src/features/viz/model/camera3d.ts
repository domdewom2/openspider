// Orbit-Kamera der 3D-Graphen (UI-098, OS-Festlegung): Linksziehen dreht, Mausrad zoomt (0,05× … 8× wie UI-090, um
// den Zeiger), Rechtsziehen verschiebt. Reine Mathematik ohne DOM: Matrizen spaltenweise (WebGL-Konvention).
import { clampZoom } from "./viewport";

export type Vec3 = [number, number, number];

export interface Camera3D {
  /** Blickziel. */
  target: Vec3;
  /** Azimut (um die y-Achse) und Polarwinkel (0 = von oben), Bogenmaß. */
  theta: number;
  phi: number;
  /** Abstand bei Zoom 1× (Szene passt ins Bild). */
  base: number;
  /** Zoomstufe k = base / Abstand, begrenzt auf 0,05 … 8. */
  k: number;
  /** Vertikaler Öffnungswinkel. */
  fov: number;
}

export const DEFAULT_FOV = Math.PI / 4;
const ROTATE_PER_PX = 0.006;
const PHI_MIN = 0.01;

export function initialCamera(radius: number, center: Vec3 = [0, 0, 0]): Camera3D {
  const r = Math.max(1, radius);
  return { target: [...center], theta: 0.6, phi: 1.1, base: (r / Math.sin(DEFAULT_FOV / 2)) * 1.05, k: 1, fov: DEFAULT_FOV };
}

export function distance(c: Camera3D): number {
  return c.base / c.k;
}

export function eye(c: Camera3D): Vec3 {
  const d = distance(c);
  const s = Math.sin(c.phi);
  return [c.target[0] + d * s * Math.sin(c.theta), c.target[1] + d * Math.cos(c.phi), c.target[2] + d * s * Math.cos(c.theta)];
}

function sub(a: Vec3, b: Vec3): Vec3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}
function cross(a: Vec3, b: Vec3): Vec3 {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}
function norm(a: Vec3): Vec3 {
  const l = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
}
function dot(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

/** Rechts- und Hoch-Vektor der Bildebene. */
export function basis(c: Camera3D): { right: Vec3; up: Vec3; forward: Vec3 } {
  const forward = norm(sub(c.target, eye(c)));
  const right = norm(cross(forward, [0, 1, 0]));
  const up = cross(right, forward);
  return { right, up, forward };
}

/** Sichtmatrix (lookAt). */
export function viewMatrix(c: Camera3D): Float32Array {
  const e = eye(c);
  const { right, up, forward } = basis(c);
  return new Float32Array([right[0], up[0], -forward[0], 0, right[1], up[1], -forward[1], 0, right[2], up[2], -forward[2], 0, -dot(right, e), -dot(up, e), dot(forward, e), 1]);
}

/** Projektionsmatrix; Nah-/Fernebene aus Abstand und Szenengröße. */
export function projMatrix(c: Camera3D, aspect: number, radius: number): Float32Array {
  const d = distance(c);
  const near = Math.max(0.01, (d - radius * 2) * 0.5, d * 0.001);
  const far = d + radius * 4 + 10;
  const f = 1 / Math.tan(c.fov / 2);
  const nf = 1 / (near - far);
  return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
}

/** Linksziehen: drehen (Polarwinkel begrenzt, kein Überschlag). */
export function rotate(c: Camera3D, dx: number, dy: number): Camera3D {
  return { ...c, theta: c.theta - dx * ROTATE_PER_PX, phi: Math.min(Math.PI - PHI_MIN, Math.max(PHI_MIN, c.phi - dy * ROTATE_PER_PX)) };
}

/** Weltlänge eines Bildpunkts in der Zielebene. */
export function worldPerPixel(c: Camera3D, height: number): number {
  return (2 * distance(c) * Math.tan(c.fov / 2)) / Math.max(1, height);
}

/** Rechtsziehen: Ziel in der Bildebene verschieben (Szene folgt dem Zeiger). */
export function pan(c: Camera3D, dx: number, dy: number, height: number): Camera3D {
  const w = worldPerPixel(c, height);
  const { right, up } = basis(c);
  return { ...c, target: [c.target[0] - (right[0] * dx - up[0] * dy) * w, c.target[1] - (right[1] * dx - up[1] * dy) * w, c.target[2] - (right[2] * dx - up[2] * dy) * w] };
}

/**
 * Mausrad: Zoom um `factor`, begrenzt auf 0,05× … 8×; der Punkt unter dem Zeiger (in der Zielebene) bleibt stehen.
 * `sx`/`sy` relativ zur Bildmitte in Bildpunkten.
 */
export function zoomAt(c: Camera3D, factor: number, sx: number, sy: number, height: number): Camera3D {
  const k = clampZoom(c.k * factor);
  if (k === c.k) return c;
  const before = worldPerPixel(c, height);
  const next = { ...c, k };
  const after = worldPerPixel(next, height);
  const { right, up } = basis(c);
  const shift = before - after;
  return { ...next, target: [c.target[0] + (right[0] * sx - up[0] * sy) * shift, c.target[1] + (right[1] * sx - up[1] * sy) * shift, c.target[2] + (right[2] * sx - up[2] * sy) * shift] };
}

/** Bildschirmkoordinaten (CSS-Pixel, Ursprung oben links) und Tiefe eines Weltpunkts; `null` hinter der Kamera. */
export function project(view: Float32Array, proj: Float32Array, p: Vec3, width: number, height: number): { x: number; y: number; depth: number } | null {
  const m = (a: Float32Array, i: number) => a[i] as number;
  const vx = m(view, 0) * p[0] + m(view, 4) * p[1] + m(view, 8) * p[2] + m(view, 12);
  const vy = m(view, 1) * p[0] + m(view, 5) * p[1] + m(view, 9) * p[2] + m(view, 13);
  const vz = m(view, 2) * p[0] + m(view, 6) * p[1] + m(view, 10) * p[2] + m(view, 14);
  if (vz >= 0) return null;
  const cx = m(proj, 0) * vx;
  const cy = m(proj, 5) * vy;
  const w = -vz;
  return { x: (cx / w * 0.5 + 0.5) * width, y: (1 - (cy / w * 0.5 + 0.5)) * height, depth: w };
}

/** Kamera auf einen Punkt richten: Ziel = Punkt, mindestens Zoom `minK` (Suche fliegt zum Knoten, UI-098). */
export function focusOn(c: Camera3D, p: Vec3, minK = 3): Camera3D {
  return { ...c, target: [...p], k: clampZoom(Math.max(c.k, minK)) };
}

/** Zwischenstand einer Kamerafahrt (t in [0, 1], weich). */
export function lerpCamera(a: Camera3D, b: Camera3D, t: number): Camera3D {
  // Ease-in-out (kubisch): ruhiger Start und weiches Ankommen.
  const u = t <= 0 ? 0 : t >= 1 ? 1 : t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
  const mix = (x: number, y: number) => x + (y - x) * u;
  return { ...b, target: [mix(a.target[0], b.target[0]), mix(a.target[1], b.target[1]), mix(a.target[2], b.target[2])], theta: mix(a.theta, b.theta), phi: mix(a.phi, b.phi), k: Math.exp(mix(Math.log(a.k), Math.log(b.k))) };
}
