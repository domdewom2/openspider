// Kraftgerichtetes Layout (UI-092): Knoten stoßen sich ab (Barnes-Hut), Baumkanten wirken als Federn, die Wurzel ist im
// Ursprung fixiert. Deterministisch: gleiche Eingabe + gleicher Startwert → bitgleiche Endkoordinaten (feste
// Rechenreihenfolge, eigener Zufallsgenerator, Abbruch nach Schrittzahl/Bewegung, nie nach Uhrzeit innerhalb eines Schritts).
// Läuft im Web Worker (force.worker.ts); dieselbe Klasse dient ohne Worker als Rückfall im Hauptthread.

export interface ForceParams {
  /** Abstoßungsstärke (> 0). */
  repulsion: number;
  /** Federlänge (zusätzlich zu den Radien beider Knoten). */
  springLength: number;
  /** Zufallsstartwert. */
  seed: number;
  /** Überlappung erlauben (Standard aus → Kollisionskraft aktiv). */
  allowOverlap: boolean;
}

export const FORCE_DEFAULTS: ForceParams = { repulsion: 60, springLength: 30, seed: 1, allowOverlap: false };

/** Abbruch: mittlere Bewegung je Schritt < 0,1 px (UI-092) oder Abkühlung beendet; Zeitgrenze im Treiber (10 s). */
export const MOVE_EPSILON = 0.1;
export const MAX_LAYOUT_MS = 10_000;
const MIN_STEPS = 60;
const ALPHA_MIN = 0.001;
const ALPHA_DECAY = 1 - Math.pow(ALPHA_MIN, 1 / 200);
const VELOCITY_DECAY = 0.4;
const THETA2 = 1.44;
const COLLIDE_STRENGTH = 1;
const COLLIDE_ITERATIONS = 2;
/** Zellmarken im Quadtree (sonst Index des ersten Punkts im Blatt). */
const EMPTY = -1;
const INTERNAL = -2;

export interface ForceInput {
  /** Elternindex je Knoten (-1 = Wurzel); Knoten in Breitensuche-Reihenfolge (Eltern vor Kindern). */
  parent: Int32Array;
  radius: Float64Array;
  /** Startpositionen x0,y0,x1,y1 … (NaN = frei wählen), z. B. aus einem früheren Layout. */
  init?: Float64Array;
  /** Fixierte Positionen (NaN = frei); die erste Wurzel ist immer im Ursprung fixiert. */
  fixed?: Float64Array;
  params: ForceParams;
}

/** Kleiner, schneller, deterministischer Zufallsgenerator (mulberry32). */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class ForceSim {
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
  private maxRadius = 0;
  // Barnes-Hut-Quadtree in typisierten Feldern (wird je Schritt neu aufgebaut).
  private cap = 0;
  private cx0 = new Float64Array(0);
  private cy0 = new Float64Array(0);
  private csize = new Float64Array(0);
  private cchild = new Int32Array(0);
  private cpoint = new Int32Array(0);
  private cmass = new Float64Array(0);
  private ccomx = new Float64Array(0);
  private ccomy = new Float64Array(0);
  private cells = 0;
  private readonly nextPoint: Int32Array;

  constructor(input: ForceInput) {
    this.n = input.parent.length;
    this.parent = input.parent;
    this.radius = input.radius;
    this.params = input.params;
    this.pos = new Float64Array(this.n * 2);
    this.vel = new Float64Array(this.n * 2);
    this.fixed = new Float64Array(this.n * 2).fill(Number.NaN);
    this.degree = new Float64Array(this.n);
    this.nextPoint = new Int32Array(this.n);
    for (let i = 0; i < this.n; i++) {
      const p = this.parent[i] as number;
      if (p >= 0) {
        this.degree[i] = (this.degree[i] as number) + 1;
        this.degree[p] = (this.degree[p] as number) + 1;
      }
      if ((this.radius[i] as number) > this.maxRadius) this.maxRadius = this.radius[i] as number;
    }
    let root = -1;
    for (let i = 0; i < this.n; i++) if ((this.parent[i] as number) < 0) { root = i; break; }
    this.root = root;
    if (input.fixed) this.fixed.set(input.fixed.subarray(0, this.n * 2));
    if (root >= 0) {
      this.fixed[root * 2] = 0;
      this.fixed[root * 2 + 1] = 0;
    }
    this.place(input.init);
  }

  /** Startpositionen: vorgegebene übernehmen, sonst um den Elternknoten in Federlänge (Zufall mit Startwert). */
  private place(init?: Float64Array): void {
    const rnd = mulberry32(this.params.seed);
    let extraRoots = 0;
    for (let i = 0; i < this.n; i++) {
      const a = rnd() * 2 * Math.PI;
      const d = rnd();
      const fx = this.fixed[i * 2] as number;
      const ix = init ? (init[i * 2] as number) : Number.NaN;
      if (!Number.isNaN(fx)) {
        this.pos[i * 2] = fx;
        this.pos[i * 2 + 1] = this.fixed[i * 2 + 1] as number;
      } else if (!Number.isNaN(ix)) {
        this.pos[i * 2] = ix;
        this.pos[i * 2 + 1] = (init as Float64Array)[i * 2 + 1] as number;
      } else {
        const p = this.parent[i] as number;
        const len = this.params.springLength + (this.radius[i] as number) + (p >= 0 ? (this.radius[p] as number) : 0);
        const bx = p >= 0 ? (this.pos[p * 2] as number) : (extraRoots += 1) * 4 * len;
        const by = p >= 0 ? (this.pos[p * 2 + 1] as number) : 0;
        this.pos[i * 2] = bx + Math.cos(a) * len * (0.5 + 0.5 * d);
        this.pos[i * 2 + 1] = by + Math.sin(a) * len * (0.5 + 0.5 * d);
      }
    }
  }

  fix(i: number, x: number, y: number): void {
    if (i < 0 || i >= this.n) return;
    this.fixed[i * 2] = x;
    this.fixed[i * 2 + 1] = y;
    this.pos[i * 2] = x;
    this.pos[i * 2 + 1] = y;
    this.vel[i * 2] = 0;
    this.vel[i * 2 + 1] = 0;
  }

  /** Fixierung lösen (Doppelklick, UI-090); die Wurzel bleibt fixiert. */
  release(i: number): void {
    if (i < 0 || i >= this.n || i === this.root) return;
    this.fixed[i * 2] = Number.NaN;
    this.fixed[i * 2 + 1] = Number.NaN;
  }

  /** Simulation nach einer Änderung wieder anwärmen. */
  reheat(alpha = 0.3): void {
    this.alpha = Math.max(this.alpha, alpha);
    this.lastMove = Infinity;
  }

  converged(): boolean {
    return this.steps >= MIN_STEPS && (this.lastMove < MOVE_EPSILON || this.alpha < ALPHA_MIN);
  }

  /** Ein Simulationsschritt; liefert die mittlere Bewegung der freien Knoten in px. */
  step(): number {
    this.alpha += (0 - this.alpha) * ALPHA_DECAY;
    this.links();
    this.manyBody();
    if (!this.params.allowOverlap) for (let k = 0; k < COLLIDE_ITERATIONS; k++) this.collide();
    let move = 0;
    let free = 0;
    const pos = this.pos;
    const vel = this.vel;
    for (let i = 0; i < this.n; i++) {
      const fx = this.fixed[i * 2] as number;
      if (!Number.isNaN(fx)) {
        pos[i * 2] = fx;
        pos[i * 2 + 1] = this.fixed[i * 2 + 1] as number;
        vel[i * 2] = 0;
        vel[i * 2 + 1] = 0;
        continue;
      }
      const vx = (vel[i * 2] as number) * (1 - VELOCITY_DECAY);
      const vy = (vel[i * 2 + 1] as number) * (1 - VELOCITY_DECAY);
      vel[i * 2] = vx;
      vel[i * 2 + 1] = vy;
      pos[i * 2] = (pos[i * 2] as number) + vx;
      pos[i * 2 + 1] = (pos[i * 2 + 1] as number) + vy;
      move += Math.sqrt(vx * vx + vy * vy);
      free++;
    }
    this.steps++;
    this.lastMove = free ? move / free : 0;
    return this.lastMove;
  }

  /** Bis zur Konvergenz oder `maxSteps` rechnen (Tests, kleine Graphen). */
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
      let x = (pos[c * 2] as number) + (vel[c * 2] as number) - (pos[p * 2] as number) - (vel[p * 2] as number);
      let y = (pos[c * 2 + 1] as number) + (vel[c * 2 + 1] as number) - (pos[p * 2 + 1] as number) - (vel[p * 2 + 1] as number);
      let l = Math.sqrt(x * x + y * y);
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
      x *= k;
      y *= k;
      vel[c * 2] = (vel[c * 2] as number) - x * bias;
      vel[c * 2 + 1] = (vel[c * 2 + 1] as number) - y * bias;
      vel[p * 2] = (vel[p * 2] as number) + x * (1 - bias);
      vel[p * 2 + 1] = (vel[p * 2 + 1] as number) + y * (1 - bias);
    }
  }

  private ensureCells(min: number): void {
    if (this.cap >= min) return;
    const cap = Math.max(min, this.cap * 2, 64);
    const grow = <T extends Float64Array | Int32Array>(old: T, make: (n: number) => T, per = 1): T => {
      const next = make(cap * per);
      next.set(old.subarray(0, Math.min(old.length, cap * per)) as never);
      return next;
    };
    this.cx0 = grow(this.cx0, (n) => new Float64Array(n));
    this.cy0 = grow(this.cy0, (n) => new Float64Array(n));
    this.csize = grow(this.csize, (n) => new Float64Array(n));
    this.cchild = grow(this.cchild, (n) => new Int32Array(n), 4);
    this.cpoint = grow(this.cpoint, (n) => new Int32Array(n));
    this.cmass = grow(this.cmass, (n) => new Float64Array(n));
    this.ccomx = grow(this.ccomx, (n) => new Float64Array(n));
    this.ccomy = grow(this.ccomy, (n) => new Float64Array(n));
    this.cap = cap;
  }

  private newCell(x0: number, y0: number, size: number): number {
    this.ensureCells(this.cells + 1);
    const c = this.cells++;
    this.cx0[c] = x0;
    this.cy0[c] = y0;
    this.csize[c] = size;
    this.cchild.fill(-1, c * 4, c * 4 + 4);
    this.cpoint[c] = EMPTY;
    this.cmass[c] = 0;
    return c;
  }

  private quadrant(c: number, x: number, y: number): number {
    const h = (this.csize[c] as number) / 2;
    return (x >= (this.cx0[c] as number) + h ? 1 : 0) + (y >= (this.cy0[c] as number) + h ? 2 : 0);
  }

  private childCell(c: number, q: number): number {
    const existing = this.cchild[c * 4 + q] as number;
    if (existing >= 0) return existing;
    const h = (this.csize[c] as number) / 2;
    const child = this.newCell((this.cx0[c] as number) + (q & 1 ? h : 0), (this.cy0[c] as number) + (q & 2 ? h : 0), h);
    this.cchild[c * 4 + q] = child;
    return child;
  }

  private buildTree(): void {
    const pos = this.pos;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (let i = 0; i < this.n; i++) {
      const x = pos[i * 2] as number;
      const y = pos[i * 2 + 1] as number;
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
    const size = Math.max(maxX - minX, maxY - minY, 1) * 1.0001 + 1;
    this.cells = 0;
    this.ensureCells(this.n * 2 + 16);
    const root = this.newCell(minX, minY, size);
    for (let i = 0; i < this.n; i++) {
      this.nextPoint[i] = -1;
      const x = pos[i * 2] as number;
      const y = pos[i * 2 + 1] as number;
      let c = root;
      for (;;) {
        const p = this.cpoint[c] as number;
        if (p === INTERNAL) {
          c = this.childCell(c, this.quadrant(c, x, y));
          continue;
        }
        if (p === EMPTY) {
          this.cpoint[c] = i;
          break;
        }
        const px = pos[p * 2] as number;
        const py = pos[p * 2 + 1] as number;
        if ((px === x && py === y) || (this.csize[c] as number) < 1e-6) {
          // Gleiche Position: als Liste am Blatt anhängen.
          this.nextPoint[i] = this.nextPoint[p] as number;
          this.nextPoint[p] = i;
          break;
        }
        this.cpoint[c] = INTERNAL;
        const moved = this.childCell(c, this.quadrant(c, px, py));
        this.cpoint[moved] = p;
        c = this.childCell(c, this.quadrant(c, x, y));
      }
    }
    // Schwerpunkte von unten nach oben (Kindzellen entstehen nach ihren Eltern).
    for (let c = this.cells - 1; c >= 0; c--) {
      let mass = 0;
      let sx = 0;
      let sy = 0;
      const p0 = this.cpoint[c] as number;
      if (p0 >= 0) {
        for (let p = p0; p >= 0; p = this.nextPoint[p] as number) {
          mass += 1;
          sx += pos[p * 2] as number;
          sy += pos[p * 2 + 1] as number;
        }
      } else {
        for (let q = 0; q < 4; q++) {
          const ch = this.cchild[c * 4 + q] as number;
          if (ch < 0) continue;
          const m = this.cmass[ch] as number;
          mass += m;
          sx += (this.ccomx[ch] as number) * m;
          sy += (this.ccomy[ch] as number) * m;
        }
      }
      this.cmass[c] = mass;
      this.ccomx[c] = mass ? sx / mass : 0;
      this.ccomy[c] = mass ? sy / mass : 0;
    }
  }

  private manyBody(): void {
    if (this.n < 2) return;
    this.buildTree();
    const pos = this.pos;
    const vel = this.vel;
    const strength = -this.params.repulsion * this.alpha;
    const stack = new Int32Array(this.cells);
    for (let i = 0; i < this.n; i++) {
      const xi = pos[i * 2] as number;
      const yi = pos[i * 2 + 1] as number;
      let vx = 0;
      let vy = 0;
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
          const d2 = dx * dx + dy * dy;
          const w = this.csize[c] as number;
          if ((w * w) / THETA2 < d2) {
            const f = (strength * mass) / Math.max(d2, 1);
            vx += dx * f;
            vy += dy * f;
          } else {
            for (let q = 3; q >= 0; q--) {
              const ch = this.cchild[c * 4 + q] as number;
              if (ch >= 0) stack[top++] = ch;
            }
          }
          continue;
        }
        for (let p = p0; p >= 0; p = this.nextPoint[p] as number) {
          if (p === i) continue;
          let dx = (pos[p * 2] as number) - xi;
          let dy = (pos[p * 2 + 1] as number) - yi;
          if (dx === 0 && dy === 0) {
            dx = (p - i) * 1e-6;
            dy = (i - p) * 1e-6;
          }
          const f = strength / Math.max(dx * dx + dy * dy, 1);
          vx += dx * f;
          vy += dy * f;
        }
      }
      vel[i * 2] = (vel[i * 2] as number) + vx;
      vel[i * 2 + 1] = (vel[i * 2 + 1] as number) + vy;
    }
  }

  // Kollisionsraster: Hashtabelle über Zellkoordinaten, per Zählsortierung in typisierten Feldern (keine Objekte je Schritt).
  private gridHead = new Int32Array(0);
  private gridNodes = new Int32Array(0);
  private gridCellX = new Int32Array(0);
  private gridCellY = new Int32Array(0);

  private collide(): void {
    const n = this.n;
    const pos = this.pos;
    const vel = this.vel;
    const cell = Math.max(1, this.maxRadius * 2);
    let size = 1;
    while (size < n * 2) size <<= 1;
    const mask = size - 1;
    if (this.gridHead.length !== size + 1) this.gridHead = new Int32Array(size + 1);
    if (this.gridNodes.length !== n) {
      this.gridNodes = new Int32Array(n);
      this.gridCellX = new Int32Array(n);
      this.gridCellY = new Int32Array(n);
    }
    const head = this.gridHead;
    const sorted = this.gridNodes;
    const cx = this.gridCellX;
    const cy = this.gridCellY;
    const hash = (gx: number, gy: number) => (Math.imul(gx, 73856093) ^ Math.imul(gy, 19349663)) & mask;
    head.fill(0);
    for (let i = 0; i < n; i++) {
      const gx = Math.floor(((pos[i * 2] as number) + (vel[i * 2] as number)) / cell);
      const gy = Math.floor(((pos[i * 2 + 1] as number) + (vel[i * 2 + 1] as number)) / cell);
      cx[i] = gx;
      cy[i] = gy;
      const h = hash(gx, gy) + 1;
      head[h] = (head[h] as number) + 1;
    }
    for (let h = 1; h <= size; h++) head[h] = (head[h] as number) + (head[h - 1] as number);
    // Zweiter Zähler: Einfügeposition je Bucket (head[h] = Anfang, wird dabei auf das Ende geschoben und danach zurückgesetzt).
    for (let i = 0; i < n; i++) {
      const h = hash(cx[i] as number, cy[i] as number);
      sorted[head[h] as number] = i;
      head[h] = (head[h] as number) + 1;
    }
    for (let h = size; h > 0; h--) head[h] = head[h - 1] as number;
    head[0] = 0;
    for (let i = 0; i < n; i++) {
      const ri = this.radius[i] as number;
      const ri2 = ri * ri;
      const gx = cx[i] as number;
      const gy = cy[i] as number;
      for (let ox = -1; ox <= 1; ox++) {
        for (let oy = -1; oy <= 1; oy++) {
          const tx = gx + ox;
          const ty = gy + oy;
          const h = hash(tx, ty);
          const end = head[h + 1] as number;
          for (let k = head[h] as number; k < end; k++) {
            const j = sorted[k] as number;
            // Nur echte Nachbarzelle (Hash-Kollisionen) und jedes Paar einmal.
            if (j <= i || cx[j] !== tx || cy[j] !== ty) continue;
            const rj = this.radius[j] as number;
            const r = ri + rj;
            let x = (pos[i * 2] as number) + (vel[i * 2] as number) - (pos[j * 2] as number) - (vel[j * 2] as number);
            let y = (pos[i * 2 + 1] as number) + (vel[i * 2 + 1] as number) - (pos[j * 2 + 1] as number) - (vel[j * 2 + 1] as number);
            let l = x * x + y * y;
            if (l >= r * r) continue;
            if (l === 0) {
              x = (j - i) * 1e-6;
              y = (i - j) * 1e-6;
              l = x * x + y * y;
            }
            l = Math.sqrt(l);
            const f = ((r - l) / l) * COLLIDE_STRENGTH;
            x *= f;
            y *= f;
            const rj2 = rj * rj;
            const share = rj2 / (ri2 + rj2);
            vel[i * 2] = (vel[i * 2] as number) + x * share;
            vel[i * 2 + 1] = (vel[i * 2 + 1] as number) + y * share;
            vel[j * 2] = (vel[j * 2] as number) - x * (1 - share);
            vel[j * 2 + 1] = (vel[j * 2 + 1] as number) - y * (1 - share);
          }
        }
      }
    }
  }
}

/** Nachrichten zwischen Fenster und Worker. */
export type ForceRequest =
  | { type: "start"; input: ForceInput; maxMs?: number }
  | { type: "fix"; index: number; x: number; y: number }
  | { type: "release"; index: number }
  | { type: "pause" }
  | { type: "resume" }
  | { type: "stop" };

export type ForceReply =
  | { type: "tick"; positions: Float64Array; steps: number; move: number }
  | { type: "done"; positions: Float64Array; steps: number; move: number; reason: "converged" | "timeout" };

/**
 * Schrittweiser Treiber für Worker und Hauptthread: rechnet in Scheiben von `sliceMs`, meldet Zwischenstände und endet
 * bei Konvergenz oder nach `maxMs` (Zeitgrenze aus UI-092). Pause hält nach der laufenden Scheibe an.
 */
export class ForceDriver {
  private sim: ForceSim | null = null;
  private started = 0;
  private paused = false;
  private running = false;
  private finished = false;
  private maxMs = MAX_LAYOUT_MS;

  constructor(
    private readonly post: (reply: ForceReply) => void,
    private readonly schedule: (fn: () => void) => void = (fn) => setTimeout(fn, 0),
    private readonly now: () => number = () => performance.now(),
    private readonly sliceMs = 12,
  ) {}

  handle(msg: ForceRequest): void {
    switch (msg.type) {
      case "start":
        this.sim = new ForceSim(msg.input);
        this.maxMs = msg.maxMs ?? MAX_LAYOUT_MS;
        this.started = this.now();
        this.paused = false;
        this.finished = false;
        this.kick();
        break;
      case "fix":
        this.sim?.fix(msg.index, msg.x, msg.y);
        this.restart();
        break;
      case "release":
        this.sim?.release(msg.index);
        this.restart();
        break;
      case "pause":
        this.paused = true;
        break;
      case "resume":
        this.paused = false;
        this.kick();
        break;
      case "stop":
        this.sim = null;
        this.running = false;
        break;
    }
  }

  private restart(): void {
    if (!this.sim) return;
    this.sim.reheat();
    if (this.finished) {
      this.finished = false;
      this.started = this.now();
    }
    this.kick();
  }

  private kick(): void {
    if (this.running || this.paused || !this.sim) return;
    this.running = true;
    this.schedule(() => this.slice());
  }

  private slice(): void {
    const sim = this.sim;
    if (!sim || this.paused) {
      this.running = false;
      return;
    }
    const end = this.now() + this.sliceMs;
    do sim.step();
    while (!sim.converged() && this.now() < end);
    const positions = sim.pos.slice();
    if (sim.converged() || this.now() - this.started >= this.maxMs) {
      this.running = false;
      this.finished = true;
      this.post({ type: "done", positions, steps: sim.steps, move: sim.lastMove, reason: sim.converged() ? "converged" : "timeout" });
      return;
    }
    this.post({ type: "tick", positions, steps: sim.steps, move: sim.lastMove });
    this.schedule(() => this.slice());
  }
}
