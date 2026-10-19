// Ankertext-Wortwolke (UI-099): Schriftgröße linear 12–64 px nach Häufigkeit, Platzierung mit d3-cloud und festem
// Startwert (deterministisch), nur waagerechte Begriffe.
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

export interface PlacedWord {
  text: string;
  count: number;
  size: number;
  x: number;
  y: number;
}

/** Häufigste zuerst, gleich häufige alphabetisch (stabile Eingabe für die Platzierung). */
export function sortTerms(words: readonly WordFreq[]): WordFreq[] {
  return [...words].sort((a, b) => b.count - a.count || (a.word < b.word ? -1 : a.word > b.word ? 1 : 0));
}

interface CloudWord {
  text: string;
  count: number;
  size: number;
  x?: number;
  y?: number;
}

/** Platziert die Begriffe in einer Fläche w × h (Mittelpunkt = 0,0); nicht platzierbare Begriffe fehlen im Ergebnis. */
export function layoutWords(words: readonly WordFreq[], width: number, height: number, fontFamily: string, seed = WORD_SEED): Promise<PlacedWord[]> {
  const sorted = sortTerms(words);
  if (!sorted.length) return Promise.resolve([]);
  const min = Math.min(...sorted.map((w) => w.count));
  const max = Math.max(...sorted.map((w) => w.count));
  const data: CloudWord[] = sorted.map((w) => ({ text: w.word, count: w.count, size: fontSize(w.count, min, max) }));
  return new Promise((resolve) => {
    cloud<CloudWord>()
      .size([Math.max(64, Math.floor(width)), Math.max(64, Math.floor(height))])
      .words(data)
      .padding(2)
      .rotate(() => 0)
      .font(fontFamily)
      .fontSize((d) => d.size)
      .random(mulberry32(seed))
      .timeInterval(Infinity)
      .on("end", (placed: CloudWord[]) => resolve(placed.map((p) => ({ text: p.text, count: p.count, size: p.size, x: p.x ?? 0, y: p.y ?? 0 }))))
      .start();
  });
}
