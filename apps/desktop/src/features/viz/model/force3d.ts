// 3D-Layout der kraftgerichteten Graphen (UI-098) als Erweiterung des Barnes-Hut-Layouts aus force.ts: Octree statt
// Quadtree, sonst dieselben Kräfte (Abstoßung, Baumkanten als Federn, fixierte Wurzel im Ursprung) und dieselbe
// Determinismusregel (feste Rechenreihenfolge, mulberry32 mit Startwert, Abbruch nur nach Schrittzahl/Bewegung).
// Ohne Kollisionskraft: Knoten sind Punkte mit Bildschirmgröße, Überlappung ist in 3D die Regel.
import { mulberry32, type ForceInput, type ForceParams, type Simulation } from "./force";

const MIN_STEPS = 60;
const ALPHA_MIN = 0.001;
const ALPHA_DECAY = 1 - Math.pow(ALPHA_MIN, 1 / 200);
const VELOCITY_DECAY = 0.4;
/** θ² des Barnes-Hut-Kriteriums (θ = 1,2 wie in 2D). */
const THETA2 = 1.44;
export const MOVE_EPSILON_3D = 0.1;
const EMPTY = -1;
const INTERNAL = -2;

export class ForceSim3D implements Simulation {
  readonly n: number;
  readonly pos: Float64Array;
  readonly vel: Float64Array;
  readonly fixed: Float64Array;
  readonly radius: Float64Array;
  readonly parent: Int32Array;
  readonly params: ForceParams;
  alpha = 1;
  steps = 0;
  lastMove = Infinity;
  private readonly degree: Float64Array;
  private readonly root: number;
  // Octree in typisierten Feldern, je Schritt neu aufgebaut.
  private cap = 0;
  private cx0 = new Float64Array(0);
  private cy0 = new Float64Array(0);
  private cz0 = new Float64Array(0);
  private csize = new Float64Array(0);
  private cchild = new Int32Array(0);
  private cpoint = new Int32Array(0);
  private cmass = new Float64Array(0);
  private ccomx = new Float64Array(0);
  private ccomy = new Float64Array(0);
  private ccomz = new Float64Array(0);
  private cells = 0;
  private stack = new Int32Array(0);
  private readonly nextPoint: Int32Array;

  constructor(input: ForceInput) {
    this.n = input.parent.length;
    this.parent = input.parent;
    this.radius = input.radius;
    this.params = input.params;
    this.pos = new Float64Array(this.n * 3);
    this.vel = new Float64Array(this.n * 3);
    this.fixed = new Float64Array(this.n * 3).fill(Number.NaN);
    this.degree = new Float64Array(this.n);
    this.nextPoint = new Int32Array(this.n);
    for (let i = 0; i < this.n; i++) {
      const p = this.parent[i] as number;
      if (p >= 0) {
        this.degree[i] = (this.degree[i] as number) + 1;
        this.degree[p] = (this.degree[p] as number) + 1;
      }
    }
    let root = -1;
    for (let i = 0; i < this.n; i++) if ((this.parent[i] as number) < 0) { root = i; break; }
    this.root = root;
    if (input.fixed) this.fixed.set(input.fixed.subarray(0, this.n * 3));
    if (root >= 0) this.fixed.fill(0, root * 3, root * 3 + 3);
    this.place(input.init);
    if (input.resume) {
      this.alpha = input.resume.alpha;
      this.steps = input.resume.steps;
      this.vel.set(input.resume.velocities.subarray(0, this.n * 3));
    }
  }

  /** Startpositionen: vorgegebene übernehmen, sonst gleichmäßig zufällig auf einer Kugel um den Elternknoten. */
  private place(init?: Float64Array): void {
    const rnd = mulberry32(this.params.seed);
    let extraRoots = 0;
    for (let i = 0; i < this.n; i++) {
      const u = rnd() * 2 - 1;
      const a = rnd() * 2 * Math.PI;
      const d = rnd();
      const fx = this.fixed[i * 3] as number;
      const ix = init ? (init[i * 3] as number) : Number.NaN;
      if (!Number.isNaN(fx)) {
        for (let k = 0; k < 3; k++) this.pos[i * 3 + k] = this.fixed[i * 3 + k] as number;
      } else if (!Number.isNaN(ix)) {
        for (let k = 0; k < 3; k++) this.pos[i * 3 + k] = (init as Float64Array)[i * 3 + k] as number;
      } else {
        const p = this.parent[i] as number;
        const len = (this.params.springLength + (this.radius[i] as number) + (p >= 0 ? (this.radius[p] as number) : 0)) * (0.5 + 0.5 * d);
        const s = Math.sqrt(1 - u * u);
        const bx = p >= 0 ? (this.pos[p * 3] as number) : (extraRoots += 1) * 4 * len;
        const by = p >= 0 ? (this.pos[p * 3 + 1] as number) : 0;
        const bz = p >= 0 ? (this.pos[p * 3 + 2] as number) : 0;
        this.pos[i * 3] = bx + s * Math.cos(a) * len;
        this.pos[i * 3 + 1] = by + s * Math.sin(a) * len;
        this.pos[i * 3 + 2] = bz + u * len;
      }
    }
  }

  fix(i: number, x: number, y: number, z = 0): void {
    if (i < 0 || i >= this.n) return;
    const v = [x, y, z];
    for (let k = 0; k < 3; k++) {
      this.fixed[i * 3 + k] = v[k] as number;
      this.pos[i * 3 + k] = v[k] as number;
      this.vel[i * 3 + k] = 0;
    }
  }

  release(i: number): void {
    if (i < 0 || i >= this.n || i === this.root) return;
    this.fixed.fill(Number.NaN, i * 3, i * 3 + 3);
  }

  reheat(alpha = 0.3): void {
    this.alpha = Math.max(this.alpha, alpha);
    this.lastMove = Infinity;
  }

  converged(): boolean {
    return this.steps >= MIN_STEPS && (this.lastMove < MOVE_EPSILON_3D || this.alpha < ALPHA_MIN);
  }

  step(): number {
    this.alpha += (0 - this.alpha) * ALPHA_DECAY;
    this.links();
    this.manyBody();
    let move = 0;
    let free = 0;
    const pos = this.pos;
    const vel = this.vel;
    for (let i = 0; i < this.n; i++) {
      const b = i * 3;
      if (!Number.isNaN(this.fixed[b] as number)) {
        for (let k = 0; k < 3; k++) {
          pos[b + k] = this.fixed[b + k] as number;
          vel[b + k] = 0;
        }
        continue;
      }
      const vx = (vel[b] as number) * (1 - VELOCITY_DECAY);
      const vy = (vel[b + 1] as number) * (1 - VELOCITY_DECAY);
      const vz = (vel[b + 2] as number) * (1 - VELOCITY_DECAY);
      vel[b] = vx;
      vel[b + 1] = vy;
      vel[b + 2] = vz;
      pos[b] = (pos[b] as number) + vx;
      pos[b + 1] = (pos[b + 1] as number) + vy;
      pos[b + 2] = (pos[b + 2] as number) + vz;
      move += Math.sqrt(vx * vx + vy * vy + vz * vz);
      free++;
    }
    this.steps++;
    this.lastMove = free ? move / free : 0;
    return this.lastMove;
  }

  run(maxSteps = 5_000): number {
    while (!this.converged() && this.steps < maxSteps) this.step();
    return this.steps;
  }

  private links(): void {
    const pos = this.pos;
    const vel = this.vel;
    for (let c = 0; c < this.n; c++) {
      const p = this.parent[c] as number;
      if (p < 0) continue;
      const cb = c * 3;
      const pb = p * 3;
      let x = (pos[cb] as number) + (vel[cb] as number) - (pos[pb] as number) - (vel[pb] as number);
      const y = (pos[cb + 1] as number) + (vel[cb + 1] as number) - (pos[pb + 1] as number) - (vel[pb + 1] as number);
      const z = (pos[cb + 2] as number) + (vel[cb + 2] as number) - (pos[pb + 2] as number) - (vel[pb + 2] as number);
      let l = Math.sqrt(x * x + y * y + z * z);
      if (l === 0) {
        x = 1e-6 * (c + 1);
        l = Math.abs(x);
      }
      const distance = this.params.springLength + (this.radius[c] as number) + (this.radius[p] as number);
      const dc = this.degree[c] as number;
      const dp = this.degree[p] as number;
      const strength = 1 / Math.min(dc, dp);
      const bias = dp / (dp + dc);
      const k = ((l - distance) / l) * this.alpha * strength;
      const fx = x * k;
      const fy = y * k;
      const fz = z * k;
      vel[cb] = (vel[cb] as number) - fx * bias;
      vel[cb + 1] = (vel[cb + 1] as number) - fy * bias;
      vel[cb + 2] = (vel[cb + 2] as number) - fz * bias;
      vel[pb] = (vel[pb] as number) + fx * (1 - bias);
      vel[pb + 1] = (vel[pb + 1] as number) + fy * (1 - bias);
      vel[pb + 2] = (vel[pb + 2] as number) + fz * (1 - bias);
    }
  }

  private ensureCells(min: number): void {
    if (this.cap >= min) return;
    const cap = Math.max(min, this.cap * 2, 64);
    const f64 = (old: Float64Array, per = 1) => {
      const next = new Float64Array(cap * per);
      next.set(old.subarray(0, Math.min(old.length, cap * per)));
      return next;
    };
    const i32 = (old: Int32Array, per = 1) => {
      const next = new Int32Array(cap * per);
      next.set(old.subarray(0, Math.min(old.length, cap * per)));
      return next;
    };
    this.cx0 = f64(this.cx0);
    this.cy0 = f64(this.cy0);
    this.cz0 = f64(this.cz0);
    this.csize = f64(this.csize);
    this.cchild = i32(this.cchild, 8);
    this.cpoint = i32(this.cpoint);
    this.cmass = f64(this.cmass);
    this.ccomx = f64(this.ccomx);
    this.ccomy = f64(this.ccomy);
    this.ccomz = f64(this.ccomz);
    this.cap = cap;
  }

  private newCell(x0: number, y0: number, z0: number, size: number): number {
    this.ensureCells(this.cells + 1);
    const c = this.cells++;
    this.cx0[c] = x0;
    this.cy0[c] = y0;
    this.cz0[c] = z0;
    this.csize[c] = size;
    this.cchild.fill(-1, c * 8, c * 8 + 8);
    this.cpoint[c] = EMPTY;
    this.cmass[c] = 0;
    return c;
  }

  private octant(c: number, x: number, y: number, z: number): number {
    const h = (this.csize[c] as number) / 2;
    return (x >= (this.cx0[c] as number) + h ? 1 : 0) + (y >= (this.cy0[c] as number) + h ? 2 : 0) + (z >= (this.cz0[c] as number) + h ? 4 : 0);
  }

  private childCell(c: number, o: number): number {
    const existing = this.cchild[c * 8 + o] as number;
    if (existing >= 0) return existing;
    const h = (this.csize[c] as number) / 2;
    const child = this.newCell((this.cx0[c] as number) + (o & 1 ? h : 0), (this.cy0[c] as number) + (o & 2 ? h : 0), (this.cz0[c] as number) + (o & 4 ? h : 0), h);
    this.cchild[c * 8 + o] = child;
    return child;
  }

  private buildTree(): void {
    const pos = this.pos;
    let minX = Infinity;
    let minY = Infinity;
    let minZ = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    let maxZ = -Infinity;
    for (let i = 0; i < this.n; i++) {
      const x = pos[i * 3] as number;
      const y = pos[i * 3 + 1] as number;
      const z = pos[i * 3 + 2] as number;
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (z < minZ) minZ = z;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
      if (z > maxZ) maxZ = z;
    }
    const size = Math.max(maxX - minX, maxY - minY, maxZ - minZ, 1) * 1.0001 + 1;
    this.cells = 0;
    this.ensureCells(this.n * 2 + 16);
    const root = this.newCell(minX, minY, minZ, size);
    for (let i = 0; i < this.n; i++) {
      this.nextPoint[i] = -1;
      const x = pos[i * 3] as number;
      const y = pos[i * 3 + 1] as number;
      const z = pos[i * 3 + 2] as number;
      let c = root;
      for (;;) {
        const p = this.cpoint[c] as number;
        if (p === INTERNAL) {
          c = this.childCell(c, this.octant(c, x, y, z));
          continue;
        }
        if (p === EMPTY) {
          this.cpoint[c] = i;
          break;
        }
        const px = pos[p * 3] as number;
        const py = pos[p * 3 + 1] as number;
        const pz = pos[p * 3 + 2] as number;
        if ((px === x && py === y && pz === z) || (this.csize[c] as number) < 1e-6) {
          this.nextPoint[i] = this.nextPoint[p] as number;
          this.nextPoint[p] = i;
          break;
        }
        this.cpoint[c] = INTERNAL;
        const moved = this.childCell(c, this.octant(c, px, py, pz));
        this.cpoint[moved] = p;
        c = this.childCell(c, this.octant(c, x, y, z));
      }
    }
    for (let c = this.cells - 1; c >= 0; c--) {
      let mass = 0;
      let sx = 0;
      let sy = 0;
      let sz = 0;
      const p0 = this.cpoint[c] as number;
      if (p0 >= 0) {
        for (let p = p0; p >= 0; p = this.nextPoint[p] as number) {
          mass += 1;
          sx += pos[p * 3] as number;
          sy += pos[p * 3 + 1] as number;
          sz += pos[p * 3 + 2] as number;
        }
      } else {
        for (let o = 0; o < 8; o++) {
          const ch = this.cchild[c * 8 + o] as number;
          if (ch < 0) continue;
          const m = this.cmass[ch] as number;
          mass += m;
          sx += (this.ccomx[ch] as number) * m;
          sy += (this.ccomy[ch] as number) * m;
          sz += (this.ccomz[ch] as number) * m;
        }
      }
      this.cmass[c] = mass;
      this.ccomx[c] = mass ? sx / mass : 0;
      this.ccomy[c] = mass ? sy / mass : 0;
      this.ccomz[c] = mass ? sz / mass : 0;
    }
  }

  private manyBody(): void {
    if (this.n < 2) return;
    this.buildTree();
    const pos = this.pos;
    const vel = this.vel;
    const strength = -this.params.repulsion * this.alpha;
    if (this.stack.length < this.cells * 8 + 8) this.stack = new Int32Array(this.cap * 8 + 8);
    const stack = this.stack;
    for (let i = 0; i < this.n; i++) {
      const xi = pos[i * 3] as number;
      const yi = pos[i * 3 + 1] as number;
      const zi = pos[i * 3 + 2] as number;
      let vx = 0;
      let vy = 0;
      let vz = 0;
      let top = 0;
      stack[top++] = 0;
      while (top > 0) {
        const c = stack[--top] as number;
        const mass = this.cmass[c] as number;
        if (mass === 0) continue;
        const p0 = this.cpoint[c] as number;
        if (p0 < 0) {
          const dx = (this.ccomx[c] as number) - xi;
          const dy = (this.ccomy[c] as number) - yi;
          const dz = (this.ccomz[c] as number) - zi;
          const d2 = dx * dx + dy * dy + dz * dz;
          const w = this.csize[c] as number;
          if ((w * w) / THETA2 < d2) {
            const f = (strength * mass) / Math.max(d2, 1);
            vx += dx * f;
            vy += dy * f;
            vz += dz * f;
          } else {
            for (let o = 7; o >= 0; o--) {
              const ch = this.cchild[c * 8 + o] as number;
              if (ch >= 0) stack[top++] = ch;
            }
          }
          continue;
        }
        for (let p = p0; p >= 0; p = this.nextPoint[p] as number) {
          if (p === i) continue;
          let dx = (pos[p * 3] as number) - xi;
          let dy = (pos[p * 3 + 1] as number) - yi;
          const dz = (pos[p * 3 + 2] as number) - zi;
          if (dx === 0 && dy === 0 && dz === 0) {
            dx = (p - i) * 1e-6;
            dy = (i - p) * 1e-6;
          }
          const f = strength / Math.max(dx * dx + dy * dy + dz * dz, 1);
          vx += dx * f;
          vy += dy * f;
          vz += dz * f;
        }
      }
      vel[i * 3] = (vel[i * 3] as number) + vx;
      vel[i * 3 + 1] = (vel[i * 3 + 1] as number) + vy;
      vel[i * 3 + 2] = (vel[i * 3 + 2] as number) + vz;
    }
  }
}
