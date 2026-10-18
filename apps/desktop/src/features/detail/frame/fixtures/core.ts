import basicFixture from "./fx-ui-basic.json";
import { MockCore } from "../../../../ipc/transport/mock";
import type { ViewSpec } from "../../../../ipc/gen/ViewSpec";
import type { ViewOpenArgs } from "../../../../ipc/gen/ViewOpenArgs";
import type { RowsArgs } from "../../../../ipc/gen/RowsArgs";
import type { LocateArgs } from "../../../../ipc/gen/LocateArgs";
import type { ViewCloseArgs } from "../../../../ipc/gen/ViewCloseArgs";
import type { ColumnMeta } from "../../../../ipc/gen/ColumnMeta";
import type { ExportDetailArgs } from "../../../../ipc/gen/ExportDetailArgs";

type RecordRow = Record<string, unknown>;
export function detailFixtureCore() {
  const fixture = structuredClone(basicFixture);
  const core = new MockCore(); let next = 0;
  const views = new Map<string, { spec: ViewSpec; columns: string[]; rows: RecordRow[] }>();
  const options = { total: undefined as number | undefined, stored: true };
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: { rendering: { mode: "javascript" }, extraction: { store_html: true }, spider: { store_uncrawlable_links: true }, link_positions: { rules: [{ position: "Editorial" }] } } }));
  core.handlers.set("detail.urlDetails", raw => (raw as { urlId: number }).urlId === 7 ? [{ name: "Address", value: "https://site.test/image.svg" }] : fixture.urlDetails);
  core.handlers.set("detail.headers", () => fixture.headers);
  core.handlers.set("detail.imagePreview", () => ({ url: "ospider://img/fixture/7", width: 800, height: 600, stored: options.stored }));
  core.handlers.set("view.open", raw => {
    const { spec, columns } = raw as ViewOpenArgs;
    const name = spec.kind === "detail" ? spec.detail : spec.kind === "inlinks" || spec.kind === "outlinks" ? spec.filter.mode === "anchors" ? "anchors" : spec.kind : spec.kind;
    let rows = name === "tab" ? [{ Address: "https://site.test/image.svg" }] : (fixture as unknown as Record<string, RecordRow[]>)[name] ?? [];
    if (spec.kind === "inlinks" || spec.kind === "outlinks") {
      if (spec.filter.position) rows = rows.filter(row => row["Link Position"] === spec.filter.position);
      if (spec.filter.anchor !== null && spec.filter.anchor !== undefined) rows = rows.filter(row => row["Anchor Text"] === spec.filter.anchor);
      if (spec.filter.excludeNofollow) rows = rows.filter(row => row.Follow !== false);
      if (spec.filter.id === "Uncrawlable") rows = rows.filter(row => row["Link Crawlability"] === "Uncrawlable");
    }
    if ((spec.kind === "inlinks" || spec.kind === "outlinks" || spec.kind === "resources" || spec.kind === "images") && spec.search) {
      const needle = spec.search.toLowerCase(); rows = rows.filter(row => Object.values(row).some(value => String(value).toLowerCase().includes(needle)));
    }
    const viewId = `detail-fixture-${++next}`;
    views.set(viewId, { spec, columns, rows });
    return { viewId, version: 1, total: options.total ?? rows.length, columns: columns.map(key => ({ key, kind: { kind: "text" }, defaultVisible: true, defaultWidth: 160, align: "left", sortable: true, searchable: ["contains"], extension: false } satisfies ColumnMeta)), state: "ready" };
  });
  core.handlers.set("view.rows", raw => {
    const args = raw as RowsArgs; const record = views.get(args.viewId)!;
    const rows = record.rows.slice(args.offset, args.offset + args.limit);
    return { viewId: args.viewId, version: args.version, offset: args.offset, columns: args.columns,
      rowKeys: rows.map((_, i) => record.spec.kind === "tab" ? 7 : i + args.offset + 1), cells: rows.map(row => args.columns.map(column => row[column] ?? null)) };
  });
  core.handlers.set("view.locate", raw => (raw as LocateArgs).urlId - 1);
  core.handlers.set("view.close", raw => { views.delete((raw as ViewCloseArgs).viewId); });
  core.handlers.set("view.copy", () => ({ rows: 1, cells: 1 }));
  core.handlers.set("export.detail", raw => { const args = raw as ExportDetailArgs; return { opId: `export-${args.format}`, labelKey: "detail-export" }; });
  return { core, options, views, fixture };
}
