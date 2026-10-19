// Deterministic contract fixture, independent of the pending K01/K04/K06 implementations.
import { MockCore } from "../../../ipc/transport/mock";
import type { TabInfo } from "../../../ipc/gen/TabInfo";
import type { ColumnMeta } from "../../../ipc/gen/ColumnMeta";
import type { ViewOpenArgs } from "../../../ipc/gen/ViewOpenArgs";
import type { RowsArgs } from "../../../ipc/gen/RowsArgs";
import type { LocateArgs } from "../../../ipc/gen/LocateArgs";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";
import type { CrawlConfigDto } from "../../../ipc/gen/CrawlConfigDto";
import data from "./u10-contract.json";
export const fixtureTabs = data.tabs as TabInfo[];
export const fixtureColumns = data.columns as ColumnMeta[];
export function tableFixture() {
  const core = new MockCore();
  const views = new Map<string, ViewSpec>();
  let config: CrawlConfigDto = { schemaVersion: 2, generator: "fixture", config: { speed: { threads: 2 }, ui: { columns: {} } } };
  let nextView = 0;
  let rows = data.rows.map(row => ({ ...row }));
  let version = 1;
  const matching = (spec?: ViewSpec) => rows.filter(r => spec?.kind !== "tab" ||
    (spec.filter !== "Client Error (4xx)" || r["Status Code"] === 404) &&
    (spec.search?.kind !== "simple" || r.Address.toLowerCase().includes(spec.search.text.toLowerCase())));
  core.handlers.set("catalog.tabs", () => fixtureTabs);
  core.handlers.set("catalog.columns", () => fixtureColumns);
  core.handlers.set("catalog.filterCounts", () => ({ version, filters: fixtureTabs.flatMap(tab => tab.filters.map(filter => ({ tab: tab.id, filter: { ...filter, count: filter.id === "All" ? rows.length : filter.count } }))) }));
  core.handlers.set("config.get", () => structuredClone(config));
  core.handlers.set("config.set", args => { config = structuredClone(args as CrawlConfigDto); return { invalidatedSteps: [], deferred: [] }; });
  core.handlers.set("view.open", args => {
    const { spec } = args as ViewOpenArgs; const viewId = `fixture-${++nextView}`; views.set(viewId, spec);
    return { viewId, version, total: matching(spec).length, state: "ready", columns: fixtureColumns };
  });
  core.handlers.set("view.close", () => undefined);
  core.handlers.set("view.rows", args => {
    const a = args as RowsArgs; const page = matching(views.get(a.viewId)).slice(a.offset, a.offset + a.limit);
    return { viewId: a.viewId, version: a.version, offset: a.offset, columns: a.columns, rowKeys: page.map(r => r.id),
      cells: page.map(r => a.columns.map(c => c === "Row" ? r.id : (r as Record<string, unknown>)[c] ?? null)) };
  });
  core.handlers.set("view.locate", args => {
    const a = args as LocateArgs; const i = matching(views.get(a.viewId)).findIndex(r => r.id === a.urlId); return i < 0 ? null : i;
  });
  core.handlers.set("view.copy", () => ({ rows: 1, bytes: 10 }));
  for (const name of ["crawl.recrawl", "crawl.remove", "export.view", "export.detail"]) core.handlers.set(name, () => ({ opId: `${name}-1` }));
  return { core, views, replaceRows(next: typeof rows) { rows = next; version++; }, config: () => config };
}
