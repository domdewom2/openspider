// Textausschnitt mit markierten Fundstellen (UI-047): alle Vorkommen des Fehlers, Positionen des Kerns in Zeichen (Codepoints).
export interface Piece { text: string; mark: boolean; first: boolean }
const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
export function markPieces(text: string, start: number, end: number): Piece[] {
  const chars = Array.from(text); const word = chars.slice(start, end).join("");
  if (!word) return [{ text, mark: false, first: false }];
  const from = chars.slice(0, start).join("").length;
  const ranges = [{ start: from, end: from + word.length }];
  // Weitere Vorkommen desselben Fehlers: nur als ganzes Wort bzw. ganze Wortfolge.
  for (const m of text.matchAll(new RegExp(`(?<![\\p{L}\\p{N}])${escapeRegExp(word)}(?![\\p{L}\\p{N}])`, "gu"))) {
    const s = m.index!; const e = s + word.length;
    if (!ranges.some(r => s < r.end && e > r.start)) ranges.push({ start: s, end: e });
  }
  ranges.sort((a, b) => a.start - b.start);
  const pieces: Piece[] = []; let at = 0;
  ranges.forEach((range, index) => {
    if (range.start > at) pieces.push({ text: text.slice(at, range.start), mark: false, first: false });
    pieces.push({ text: text.slice(range.start, range.end), mark: true, first: index === 0 }); at = range.end;
  });
  if (at < text.length) pieces.push({ text: text.slice(at), mark: false, first: false });
  return pieces;
}
