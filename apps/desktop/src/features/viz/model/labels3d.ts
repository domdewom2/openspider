// Auswahl der Beschriftungskandidaten im 3D-Graphen (M3-V02, Fable-Fixrunde): statt je Bild alle n Knoten zu
// projizieren und vollständig zu sortieren, nur die Tiefe (eine Zeile der Sichtmatrix) berechnen und die k nächsten per
// Max-Heap auswählen; projiziert wird nur, wer näher liegt als der bisher fernste Kandidat.

/**
 * Indizes der höchstens `k` nächsten Knoten mit Tiefe > 0, die `accept` (z. B. „im Bild“) erfüllen, aufsteigend nach
 * Tiefe; bei gleicher Tiefe kleinerer Index zuerst (deterministisch).
 */
export function nearestK(n: number, k: number, depthOf: (i: number) => number, accept: (i: number) => boolean): number[] {
  if (k <= 0 || n <= 0) return [];
  const idx = new Int32Array(k);
  const dep = new Float64Array(k);
  let size = 0;
  // Max-Heap nach (Tiefe, Index): Wurzel = fernster Kandidat.
  const greater = (a: number, b: number) => (dep[a] as number) > (dep[b] as number) || ((dep[a] as number) === (dep[b] as number) && (idx[a] as number) > (idx[b] as number));
  const swap = (a: number, b: number) => {
    const ti = idx[a] as number;
    idx[a] = idx[b] as number;
    idx[b] = ti;
    const td = dep[a] as number;
    dep[a] = dep[b] as number;
    dep[b] = td;
  };
  const up = (j: number) => {
    while (j > 0) {
      const p = (j - 1) >> 1;
      if (!greater(j, p)) break;
      swap(j, p);
      j = p;
    }
  };
  const down = (j: number) => {
    for (;;) {
      const l = 2 * j + 1;
      const r = l + 1;
      let m = j;
      if (l < size && greater(l, m)) m = l;
      if (r < size && greater(r, m)) m = r;
      if (m === j) return;
      swap(j, m);
      j = m;
    }
  };
  for (let i = 0; i < n; i++) {
    const d = depthOf(i);
    if (!(d > 0)) continue;
    if (size === k && (d > (dep[0] as number) || (d === dep[0] && i > (idx[0] as number)))) continue;
    if (!accept(i)) continue;
    if (size < k) {
      idx[size] = i;
      dep[size] = d;
      size++;
      up(size - 1);
    } else {
      idx[0] = i;
      dep[0] = d;
      down(0);
    }
  }
  const out = Array.from({ length: size }, (_, j) => j).sort((a, b) => (dep[a] as number) - (dep[b] as number) || (idx[a] as number) - (idx[b] as number));
  return out.map((j) => idx[j] as number);
}

/** Tiefe (Abstand vor der Kamera) eines Punkts aus der dritten Zeile der Sichtmatrix (spaltenweise gespeichert). */
export function viewDepth(view: Float32Array, x: number, y: number, z: number): number {
  return -((view[2] as number) * x + (view[6] as number) * y + (view[10] as number) * z + (view[14] as number));
}
