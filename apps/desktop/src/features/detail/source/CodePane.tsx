// Nur lesende Code-Ansicht auf CodeMirror 6 (UI-041): Zeilennummern, Viewport-Rendering, Hervorhebung, Diff-Dekorationen, Suchtreffer.
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { EditorSelection, EditorState, StateEffect, StateField, type Range } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, ViewPlugin, gutter, lineNumbers, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { findMatches, type SearchResult } from "./model";
import { tokenizeHtml } from "./highlight";

export type DiffKind = "added" | "removed";
export interface MarkRange { from: number; to: number }
export interface Overlay {
  lineKinds: Map<number, DiffKind>;
  wordMarks: (MarkRange & { kind: DiffKind })[];
  matches: MarkRange[];
  current: number;
  target: MarkRange | null;
  highlight: boolean;
}
export const EMPTY_OVERLAY: Overlay = { lineKinds: new Map(), wordMarks: [], matches: [], current: -1, target: null, highlight: true };
const setOverlay = StateEffect.define<Partial<Overlay>>();
const overlayField = StateField.define<Overlay>({
  create: () => EMPTY_OVERLAY,
  update(value, tr) {
    let next = value;
    for (const effect of tr.effects) if (effect.is(setOverlay)) next = { ...next, ...effect.value };
    return next;
  },
});
/** Index des ersten Elements mit `to > from` in einer nach `from` sortierten Liste. */
function firstAfter(list: readonly MarkRange[], from: number): number {
  let lo = 0; let hi = list.length;
  while (lo < hi) { const mid = (lo + hi) >> 1; if (list[mid]!.to <= from) lo = mid + 1; else hi = mid; }
  return lo;
}
function build(view: EditorView): DecorationSet {
  const o = view.state.field(overlayField); const ranges: Range<Decoration>[] = []; const doc = view.state.doc;
  for (const { from, to } of view.visibleRanges) {
    if (o.lineKinds.size > 0) for (let pos = from; pos <= to;) { const line = doc.lineAt(pos); const kind = o.lineKinds.get(line.number); if (kind) ranges.push(Decoration.line({ class: `os-src-line-${kind}` }).range(line.from)); pos = line.to + 1; }
    if (o.highlight) for (const tok of tokenizeHtml(view.state.sliceDoc(from, to), from)) ranges.push(Decoration.mark({ class: `os-tok-${tok.cls}` }).range(tok.from, tok.to));
    for (let i = firstAfter(o.wordMarks, from); i < o.wordMarks.length && o.wordMarks[i]!.from < to; i++) {
      const mark = o.wordMarks[i]!; ranges.push(Decoration.mark({ class: `os-src-word-${mark.kind}` }).range(mark.from, mark.to));
    }
    for (let i = firstAfter(o.matches, from); i < o.matches.length && o.matches[i]!.from < to; i++) {
      const match = o.matches[i]!; ranges.push(Decoration.mark({ class: i === o.current ? "os-src-match os-src-match-current" : "os-src-match" }).range(match.from, match.to));
    }
  }
  if (o.target && o.target.to <= doc.length) ranges.push(Decoration.mark({ class: "os-src-target" }).range(o.target.from, o.target.to));
  return Decoration.set(ranges, true);
}
const overlayPlugin = ViewPlugin.fromClass(class {
  decorations: DecorationSet;
  constructor(view: EditorView) { this.decorations = build(view); }
  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged || update.transactions.some(tr => tr.effects.some(e => e.is(setOverlay)))) this.decorations = build(update.view);
  }
}, { decorations: plugin => plugin.decorations });

class DiffMarker extends GutterMarker {
  constructor(readonly kind: DiffKind) { super(); }
  eq(other: DiffMarker) { return other.kind === this.kind; }
  toDOM() { const span = document.createElement("span"); span.className = `os-src-marker os-src-marker-${this.kind}`; span.textContent = this.kind === "added" ? "+" : "−"; span.setAttribute("aria-hidden", "true"); return span; }
}
const markers = { added: new DiffMarker("added"), removed: new DiffMarker("removed") };
const diffGutter = gutter({
  class: "os-src-diff-gutter",
  lineMarker: (view, line) => { const kind = view.state.field(overlayField).lineKinds.get(view.state.doc.lineAt(line.from).number); return kind ? markers[kind] : null; },
  lineMarkerChange: update => update.transactions.some(tr => tr.effects.some(e => e.is(setOverlay))),
  initialSpacer: () => markers.added,
});

export interface CodePaneHandle {
  reset(): void;
  append(text: string): void;
  setOverlay(overlay: Partial<Overlay>): void;
  getText(): string;
  lineCount(): number;
  lineAt(pos: number): number;
  findMatches(search: string, regexp: boolean): SearchResult;
  /** Position am Zeilenanfang (1-basiert, auf den Bereich begrenzt). */
  lineStart(line: number): number;
  scrollTo(pos: number, select?: MarkRange): void;
  scroller(): HTMLElement | null;
}
export interface CodePaneProps {
  label: string;
  wrap: boolean;
  highlight: boolean;
  onFocus?: () => void;
  onScroll?: () => void;
}
export const CodePane = forwardRef<CodePaneHandle, CodePaneProps>(function CodePane({ label, wrap, highlight, onFocus, onScroll }, ref) {
  const host = useRef<HTMLDivElement>(null); const view = useRef<EditorView | null>(null);
  const callbacks = useRef({ onFocus, onScroll }); callbacks.current = { onFocus, onScroll };
  useEffect(() => {
    const parent = host.current; if (!parent) return;
    const editor = new EditorView({
      parent,
      state: EditorState.create({ doc: "", extensions: [
        lineNumbers(), diffGutter, overlayField, overlayPlugin, EditorState.readOnly.of(true),
        ...(wrap ? [EditorView.lineWrapping] : []),
        EditorView.contentAttributes.of({ "aria-label": label, "aria-readonly": "true" }),
        EditorView.domEventHandlers({ focus: () => { callbacks.current.onFocus?.(); return false; } }),
      ] }),
    });
    view.current = editor;
    editor.dispatch({ effects: setOverlay.of({ highlight }) });
    const scrolled = () => callbacks.current.onScroll?.();
    editor.scrollDOM.addEventListener("scroll", scrolled, { passive: true });
    return () => { editor.scrollDOM.removeEventListener("scroll", scrolled); editor.destroy(); view.current = null; };
    // Die Erweiterungen hängen nur von Beschriftung und Umbruch ab; ein Wechsel baut die Ansicht neu auf.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [label, wrap]);
  useEffect(() => { view.current?.dispatch({ effects: setOverlay.of({ highlight }) }); }, [highlight]);
  useImperativeHandle(ref, () => ({
    reset() { const v = view.current; if (v) v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: "" }, effects: setOverlay.of({ ...EMPTY_OVERLAY, highlight, lineKinds: new Map() }) }); },
    append(text) { const v = view.current; if (v) v.dispatch({ changes: { from: v.state.doc.length, insert: text } }); },
    setOverlay(overlay) { view.current?.dispatch({ effects: setOverlay.of(overlay) }); },
    getText: () => view.current?.state.doc.toString() ?? "",
    lineCount: () => view.current?.state.doc.lines ?? 0,
    lineAt: pos => view.current?.state.doc.lineAt(Math.min(pos, view.current.state.doc.length)).number ?? 1,
    findMatches: (search, regexp) => view.current ? findMatches(view.current.state.doc, search, regexp) : { matches: [], total: 0 },
    lineStart: line => { const doc = view.current?.state.doc; return doc ? doc.line(Math.min(Math.max(1, line), doc.lines)).from : 0; },
    scrollTo(pos, select) {
      const v = view.current; if (!v) return;
      const at = Math.min(pos, v.state.doc.length);
      v.dispatch({ selection: select ? EditorSelection.range(select.from, Math.min(select.to, v.state.doc.length)) : EditorSelection.cursor(at), effects: EditorView.scrollIntoView(at, { y: "center" }) });
    },
    scroller: () => view.current?.scrollDOM ?? null,
  }), [highlight]);
  return <div ref={host} className="os-src-pane" data-testid="code-pane" />;
});
