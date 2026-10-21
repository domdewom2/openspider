// Testhilfen der Fach-Detailtabs (nur vitest): Raster als Tabelle, Fixture-Kern, CodeMirror-Stubs für jsdom.
import { useEffect, useState } from "react";
import { act } from "@testing-library/react";
import { vi } from "vitest";
import type { DataGridProps } from "../../../grid/DataGrid";
import { SelectionModel } from "../../../state/selection";
import { createClient, setClient } from "../../../ipc/client";
import { setLanguage } from "../../../i18n";
import { useSession } from "../../../state/session";
import { setDetailSelection } from "../frame/selection";
import { fachFixtureCore } from "./fixtures/core";
import { CRAWL_ID } from "./fixtures/data";

/** Ersetzt nur die Canvas-Fläche; Datenquelle, Blockbildung und Auswahl bleiben die echten. */
export function FixtureGrid(p: DataGridProps) {
  const [, refresh] = useState(0);
  useEffect(() => p.source.subscribe(() => refresh(n => n + 1)), [p.source]);
  return <table aria-label={p.ariaLabel} data-total={p.source.total}><thead><tr>{p.columns.filter(c => c.visible).map(c => <th key={c.key}>{c.key}</th>)}</tr></thead><tbody>
    {Array.from({ length: Math.min(p.source.total, 50) }, (_, row) => <tr key={row} data-testid={`row-${row}`}
      onClick={() => { const id = p.source.rowKey(row); if (id !== undefined) p.onSelectionChange(SelectionModel.empty().select(id)); }}>
      {p.columns.filter(c => c.visible).map(c => <td key={c.key} onContextMenu={event => { event.preventDefault(); p.onContextMenu({ column: c.key, row, x: 1, y: 1 }); }}>{String(p.source.getCell(p.source.columns.findIndex(meta => meta.key === c.key), row) ?? "")}</td>)}
    </tr>)}
  </tbody></table>;
}
export async function gridMock(importOriginal: () => Promise<typeof import("../../../grid/DataGrid")>) {
  const original = await importOriginal();
  return { ...original, DataGrid: FixtureGrid };
}

/** jsdom kennt Range-Messung nicht, die CodeMirror beim Zeichnen braucht. */
export function stubRanges() {
  const rect = { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON: () => ({}) } as DOMRect;
  const list = Object.assign([] as unknown as DOMRectList, { item: () => null });
  Range.prototype.getBoundingClientRect = () => rect;
  Range.prototype.getClientRects = () => list;
  (document as unknown as { elementFromPoint: () => null }).elementFromPoint = () => null;
}

export const selection = { urlIds: [1], focus: 1, topTab: "Internal", crawlId: CRAWL_ID };
export function setup(initial?: Parameters<typeof fachFixtureCore>[0]) {
  const fixture = fachFixtureCore(initial); setClient(createClient(fixture.core.transport()));
  const actions = { navigate: vi.fn(), back: vi.fn(), openBrowser: vi.fn(), export: vi.fn(), reportError: vi.fn() };
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockResolvedValue(undefined) } });
  return { ...fixture, actions };
}
export async function teardown() {
  setClient(null); setDetailSelection({ urlIds: [] }); useSession.setState({ seq: -1, crawl: null, views: {}, ops: {}, analysis: null });
}
export async function lang(code: "en" | "de") { await setLanguage(code); }
/** Kleine Schritte: Effekte, die nach einem Zustandswechsel neue Zeitgeber setzen, laufen erst nach dem Ende von `act`. */
export async function settle(ms = 120) {
  if (ms === 0) { await act(async () => { await vi.advanceTimersByTimeAsync(0); }); return; }
  for (let spent = 0; spent < ms; spent += 50) await act(async () => { await vi.advanceTimersByTimeAsync(Math.min(50, ms - spent)); });
}
export const callsOf = (core: ReturnType<typeof fachFixtureCore>["core"], cmd: string) => core.calls.filter(c => c.cmd === cmd).map(c => c.args);
