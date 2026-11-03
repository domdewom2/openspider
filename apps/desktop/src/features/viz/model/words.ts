// Wortwolken (UI-099/UI-100): Schriftgröße linear 12–64 px nach Häufigkeit, Platzierung mit d3-cloud (Archimedes-Spirale
// von der Mitte, dichte Packung) und festem Startwert (deterministisch), nur waagerechte Begriffe. Häufigkeitsstufen
// bestimmen Schriftstärke und Farbe (M3-V02, Optik).
import cloud from "d3-cloud";
import type { WordFreq } from "../../../ipc/gen/WordFreq";
import { mulberry32 } from "./force";

export const MIN_FONT = 12;
export const MAX_FONT = 64;
export const WORD_SEED = 42;

/** Linear zwischen min (12 px) und max (64 px); bei nur einer Häufigkeit erhalten alle die Maximalgröße. */
export function fontSize(count: number, minCount: number, maxCount: number): number {
  if (maxCount <= minCount) return MAX_FONT;
  return MIN_FONT + ((count - minCount) / (maxCount - minCount)) * (MAX_FONT - MIN_FONT);
}

/** Häufigkeitsstufe nach Rang: 0 = obere 6 %, 1 = bis 20 %, 2 = bis 50 %, 3 = Rest. */
export type WordTier = 0 | 1 | 2 | 3;
export const TIER_WEIGHT: Record<WordTier, number> = { 0: 700, 1: 600, 2: 500, 3: 400 };

export function tierOf(rank: number, total: number): WordTier {
  const q = total <= 1 ? 0 : rank / total;
  return q < 0.06 ? 0 : q < 0.2 ? 1 : q < 0.5 ? 2 : 3;
}

export interface PlacedWord {
  text: string;
  count: number;
  size: number;
  tier: WordTier;
  weight: number;
  x: number;
  y: number;
  /** Ausdehnung des Begriffs relativ zu (x, y) aus der Platzierung (Trefferfläche, ohne Textmessung in der Oberfläche). */
  box: { x0: number; y0: number; x1: number; y1: number };
}

/** Häufigste zuerst, gleich häufige alphabetisch (stabile Eingabe für die Platzierung). */
export function sortTerms(words: readonly WordFreq[]): WordFreq[] {
  return [...words].sort((a, b) => b.count - a.count || (a.word < b.word ? -1 : a.word > b.word ? 1 : 0));
}

interface CloudWord {
  text: string;
  count: number;
  size: number;
  tier: WordTier;
  weight: number;
  x?: number;
  y?: number;
  x0?: number;
  y0?: number;
  x1?: number;
  y1?: number;
}

/** Platziert die Begriffe in einer Fläche w × h (Mittelpunkt = 0,0); nicht platzierbare Begriffe fehlen im Ergebnis. */
export function layoutWords(words: readonly WordFreq[], width: number, height: number, fontFamily: string, seed = WORD_SEED): Promise<PlacedWord[]> {
  const sorted = sortTerms(words);
  if (!sorted.length) return Promise.resolve([]);
  const min = Math.min(...sorted.map((w) => w.count));
  const max = Math.max(...sorted.map((w) => w.count));
  const data: CloudWord[] = sorted.map((w, i) => {
    const tier = tierOf(i, sorted.length);
    return { text: w.word, count: w.count, size: fontSize(w.count, min, max), tier, weight: TIER_WEIGHT[tier] };
  });
  return new Promise((resolve) => {
    cloud<CloudWord>()
      .size([Math.max(64, Math.floor(width)), Math.max(64, Math.floor(height))])
      .words(data)
      .padding(1)
      .spiral("archimedean")
      .rotate(() => 0)
      .font(fontFamily)
      .fontWeight((d) => d.weight)
      .fontSize((d) => d.size)
      .random(mulberry32(seed))
      .timeInterval(Infinity)
      .on("end", (placed: CloudWord[]) =>
        resolve(
          placed.map((p) => ({
            text: p.text,
            count: p.count,
            size: p.size,
            tier: p.tier,
            weight: p.weight,
            x: p.x ?? 0,
            y: p.y ?? 0,
            box: { x0: p.x0 ?? -p.size, y0: p.y0 ?? -p.size, x1: p.x1 ?? p.size, y1: p.y1 ?? p.size / 2 },
          })),
        ),
      )
      .start();
  });
}

/** Ausdehnung der platzierten Begriffe (relativ zur Mitte). */
export function wordBounds(placed: readonly PlacedWord[]): { x0: number; y0: number; x1: number; y1: number } {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const p of placed) {
    x0 = Math.min(x0, p.x + p.box.x0);
    y0 = Math.min(y0, p.y + p.box.y0);
    x1 = Math.max(x1, p.x + p.box.x1);
    y1 = Math.max(y1, p.y + p.box.y1);
  }
  return Number.isFinite(x0) ? { x0, y0, x1, y1 } : { x0: 0, y0: 0, x1: 0, y1: 0 };
}

/** Zentriert die Wolke und füllt die Fläche (höchstens 2,5-fach vergrößert, 4 % Rand). */
export function fitWords(placed: readonly PlacedWord[], width: number, height: number): { k: number; cx: number; cy: number } {
  const b = wordBounds(placed);
  const bw = Math.max(1, b.x1 - b.x0);
  const bh = Math.max(1, b.y1 - b.y0);
  const k = Math.min(2.5, (width * 0.92) / bw, (height * 0.92) / bh);
  return { k, cx: (b.x0 + b.x1) / 2, cy: (b.y0 + b.y1) / 2 };
}
