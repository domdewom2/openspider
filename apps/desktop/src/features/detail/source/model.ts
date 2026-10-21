// Diff-Auswertung und Suche der Quelltext-Ansicht (UI-041). Der Kern berechnet den Diff (Myers, `similar`); hier werden nur die
// Blöcke auf Zeilen bzw. Wörter beider Seiten abgebildet. Blockanfänge (`oldStart`/`newStart`) sind 1-basiert: Zeilennummer im
// Zeilenmodus, Wortnummer (Wörter = Folgen ohne Unicode-Leerraum) im Wortmodus.
import { SearchQuery } from "@codemirror/search";
import type { Text } from "@codemirror/state";
import type { DiffHunks } from "../../../ipc/gen/DiffHunks";
import { tokenize } from "../duplicates/diff";
import type { DiffKind, MarkRange } from "./CodePane";

export interface Change { left: number; right: number }
export interface DiffMarks {
  /** Markierte Zeilen (1-basiert) je Seite. */
  leftLines: Map<number, DiffKind>; rightLines: Map<number, DiffKind>;
  /** Markierte Wörter (0-basiert) je Seite. */
  leftWords: number[]; rightWords: number[];
  /** Beginn jeder zusammenhängenden Änderung in Zeilen bzw. Wörtern (1-basiert). */
  changes: Change[];
}
export function diffMarks(hunks: DiffHunks): DiffMarks {
  const out: DiffMarks = { leftLines: new Map(), rightLines: new Map(), leftWords: [], rightWords: [], changes: [] };
  for (const hunk of hunks.hunks) {
    let left = hunk.oldStart; let right = hunk.newStart; let inChange = false;
    for (const line of hunk.lines) {
      if (line.kind === "context") { left++; right++; inChange = false; continue; }
      if (!inChange) { out.changes.push({ left, right }); inChange = true; }
      if (line.kind === "removed") { out.leftLines.set(left, "removed"); out.leftWords.push(left - 1); left++; } else { out.rightLines.set(right, "added"); out.rightWords.push(right - 1); right++; }
    }
  }
  return out;
}
/** Wortmarkierungen als Bereiche im Dokument; zusätzlich die Zeilen, in denen geänderte Wörter stehen (Randmarkierung). */
export function wordRanges(text: string, words: readonly number[], kind: DiffKind, lineOf: (pos: number) => number): { ranges: (MarkRange & { kind: DiffKind })[]; lines: Map<number, DiffKind>; tokens: ReturnType<typeof tokenize> } {
  const tokens = tokenize(text); const ranges: (MarkRange & { kind: DiffKind })[] = []; const lines = new Map<number, DiffKind>();
  for (const index of [...words].sort((a, b) => a - b)) { const token = tokens[index]; if (!token) continue; ranges.push({ from: token.start, to: token.end, kind }); lines.set(lineOf(token.start), kind); }
  return { ranges, lines, tokens };
}
export interface SearchResult { matches: MarkRange[]; total: number; error?: boolean }
export const MATCH_LIMIT = 100_000;
/** Trefferpositionen der Suche in einem Dokument (ohne Beachtung der Groß-/Kleinschreibung). */
export function findMatches(doc: Text, search: string, regexp: boolean): SearchResult {
  if (search === "") return { matches: [], total: 0 };
  if (regexp) { try { new RegExp(search); } catch { return { matches: [], total: 0, error: true }; } }
  const query = new SearchQuery({ search, regexp, caseSensitive: false });
  if (!query.valid) return { matches: [], total: 0, error: regexp };
  const cursor = query.getCursor(doc); const matches: MarkRange[] = [];
  for (let step = cursor.next(); !step.done && matches.length < MATCH_LIMIT; step = cursor.next()) {
    if (step.value.to > step.value.from) matches.push({ from: step.value.from, to: step.value.to });
  }
  return { matches, total: matches.length };
}
/** Stelle eines HTML-Ausschnitts im Dokument: erst der ganze Ausschnitt, dann sein öffnendes Tag (UI-048 „Im gerenderten HTML anzeigen“). */
export function findSnippet(text: string, snippet: string): MarkRange | null {
  const whole = snippet.trim(); if (!whole) return null;
  for (const candidate of [whole, /^<[^>]+>/.exec(whole)?.[0]]) {
    if (!candidate) continue;
    const index = text.indexOf(candidate);
    if (index !== -1) return { from: index, to: index + candidate.length };
  }
  return null;
}
