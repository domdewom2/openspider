// Wort-Diff nach Myers über die Token des Inhaltsbereichs (ONPAGE-145, ONPAGE-141: Token = Folge ohne Unicode-Leerraum).
export interface Token { text: string; start: number; end: number }
export function tokenize(text: string): Token[] {
  return [...text.matchAll(/\S+/gu)].map(m => ({ text: m[0], start: m.index!, end: m.index! + m[0].length }));
}
export interface WordDiff { removed: boolean[]; added: boolean[]; removedCount: number; addedCount: number }
/** Obergrenze der Editierdistanz; darüber gilt der Rest als vollständig ersetzt (Speicher O(D²)). */
const MAX_EDIT_DISTANCE = 1500;

/** Kürzeste Editierfolge (Myers, O(ND)); gemeinsame Anfänge und Enden werden vorab abgetrennt. */
export function diffWords(left: readonly string[], right: readonly string[]): WordDiff {
  let prefix = 0;
  while (prefix < left.length && prefix < right.length && left[prefix] === right[prefix]) prefix++;
  let suffix = 0;
  while (suffix < left.length - prefix && suffix < right.length - prefix && left[left.length - 1 - suffix] === right[right.length - 1 - suffix]) suffix++;
  const a = left.slice(prefix, left.length - suffix); const b = right.slice(prefix, right.length - suffix);
  const removed = new Array<boolean>(left.length).fill(false); const added = new Array<boolean>(right.length).fill(false);
  const n = a.length; const m = b.length; const max = Math.min(n + m, MAX_EDIT_DISTANCE);
  const mark = (side: boolean[], index: number, offset = prefix) => { side[offset + index] = true; };
  if (n === 0 || m === 0) {
    for (let i = 0; i < n; i++) mark(removed, i); for (let j = 0; j < m; j++) mark(added, j);
  } else {
    const offset = max + 1; const trace: Int32Array[] = []; const v = new Int32Array(2 * max + 3); let found = -1;
    outer: for (let d = 0; d <= max; d++) {
      trace.push(v.slice());
      for (let k = -d; k <= d; k += 2) {
        let x = k === -d || (k !== d && v[offset + k - 1]! < v[offset + k + 1]!) ? v[offset + k + 1]! : v[offset + k - 1]! + 1;
        let y = x - k;
        while (x < n && y < m && a[x] === b[y]) { x++; y++; }
        v[offset + k] = x;
        if (x >= n && y >= m) { found = d; break outer; }
      }
    }
    if (found < 0) { for (let i = 0; i < n; i++) mark(removed, i); for (let j = 0; j < m; j++) mark(added, j); }
    else {
      let x = n; let y = m;
      for (let d = found; d > 0; d--) {
        const previous = trace[d]!; const k = x - y;
        const down = k === -d || (k !== d && previous[offset + k - 1]! < previous[offset + k + 1]!);
        const previousK = down ? k + 1 : k - 1;
        const previousX = previous[offset + previousK]!; const previousY = previousX - previousK;
        while (x > previousX && y > previousY) { x--; y--; }
        if (down) { y--; mark(added, y); } else { x--; mark(removed, x); }
      }
    }
  }
  return { removed, added, removedCount: removed.filter(Boolean).length, addedCount: added.filter(Boolean).length };
}
