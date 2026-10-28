import { useMemo, useState } from "react";
import { ViewDataSource } from "../../data/ViewDataSource";
import { DataGrid, defaultColumns, type ColumnState } from "../../grid/DataGrid";
import { currentLanguage } from "../../i18n";
import type { SortKey } from "../../ipc/gen/SortKey";
import { SelectionModel } from "../../state/selection";
import { usePrefs } from "../../state/prefs";
import { currentPlatform } from "../../ui/platform";
import { FONT_STACKS, SIZES, resolveTokens } from "../../ui/tokens/tokens";
import { resolveTheme, systemPrefersDark } from "../../ui/tokens/theme";
import { ColumnsMenu } from "../table/ColumnsMenu";
import type { ComparisonPage } from "./model";

/** The comparison command already returns a bounded page. Keep its keys and raw values;
 * never send this session's synthetic view id to the ordinary view commands. */
export class ComparisonPageSource extends ViewDataSource {
  constructor(readonly page: ComparisonPage) {
    super({ viewId: page.viewId, version: page.version, total: page.cells.length, state: "ready",
      columns: page.columns.map(key => ({ key, kind: { kind: "text" }, defaultWidth: /Address|Source|Destination/.test(key) ? 280 : 180,
        defaultVisible: true, align: "left", sortable: true, searchable: [], extension: false })) }, {
      rows: async args => ({ ...args, rowKeys: page.rowKeys.slice(args.offset, args.offset + args.limit),
        cells: page.cells.slice(args.offset, args.offset + args.limit).map(row => args.columns.map(key => row[page.columns.indexOf(key)])) }),
      locate: async args => { const index = page.rowKeys.indexOf(args.urlId); return index < 0 ? null : index; },
      close: async () => undefined,
      copy: async args => {
        const selection = args.selection;
        const rows = page.cells.filter((_, i) => selection.kind === "all" ? !selection.except.includes(page.rowKeys[i]) : selection.ids.includes(page.rowKeys[i]));
        const indices = args.columns.map(key => page.columns.indexOf(key));
        const text = [...(args.withHeader ? [args.columns] : []), ...rows.map(row => indices.map(i => String(row[i] ?? "")))].map(row => row.join("\t")).join("\n");
        await navigator.clipboard.writeText(text);
        return { rows: rows.length, bytes: new TextEncoder().encode(text).length };
      },
    });
  }
  override getCell(column: number, row: number) { return String(this.page.cells[row]?.[column] ?? ""); }
  override rowKey(row: number) { return this.page.rowKeys[row]; }
  override async ensureRange() {}
  override async ensureRow() {}
  override async locate(key: number) { const row = this.page.rowKeys.indexOf(key); return row < 0 ? null : row; }
  override async keys(first: number, last: number) { return this.page.rowKeys.slice(first, last + 1); }
  override async dispose() {}
}

export function ComparisonGrid({ page, label, sort, onSort, onSelect, onError, onExport, configuration }: {
  configuration: Map<string, ColumnState[]>; page: ComparisonPage; label: string; sort?: SortKey; onSort: (sort: SortKey) => void;
  onSelect?: (key: number) => void; onError: (error: unknown) => void; onExport: () => void;
}) {
  const source = useMemo(() => new ComparisonPageSource(page), [page]);
  const schema = page.columns.join("\0");
  const [columns, updateColumns] = useState<ColumnState[]>(() => configuration.get(schema) ?? defaultColumns(source.columns));
  const setColumns = (next: ColumnState[]) => { configuration.set(schema, next); updateColumns(next); };
  const [selection, setSelection] = useState(SelectionModel.empty);
  const appearance = usePrefs(s => s.appearance);
  const select = (next: SelectionModel) => { setSelection(next); if (next.focus !== undefined) onSelect?.(next.focus); };
  return <>
    <ColumnsMenu columns={columns} onChange={setColumns} onReset={() => setColumns(defaultColumns(source.columns))} />
    <div style={{ height: 360, minWidth: 0 }}>
      <DataGrid source={source} columns={columns.map(c => ({ ...c, sort: sort?.column === c.key ? sort.dir : undefined }))}
        selection={selection} onSelectionChange={select} onSortChange={next => { if (next[0]) onSort(next[0]); }}
        onColumnsChange={setColumns} onContextMenu={() => undefined} onActivate={key => select(SelectionModel.empty().select(key))}
        onError={onError} confirmLargeCopy={async () => false} onExportSelection={onExport}
        rowHeight={SIZES.rowHeight[appearance.density]} locale={currentLanguage()} ariaLabel={label}
        theme={{ tokens: resolveTokens(resolveTheme(appearance.theme, systemPrefersDark()), appearance.accent),
          fontFamily: FONT_STACKS[currentPlatform()].ui, fontSize: SIZES.fontTable[appearance.density], zebra: appearance.zebra }} />
    </div>
  </>;
}
