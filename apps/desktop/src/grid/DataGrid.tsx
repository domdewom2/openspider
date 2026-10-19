import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import DataEditor, { CompactSelection, type DataEditorRef, type GridSelection, type Rectangle, type Theme } from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import { LOADING, type ViewDataSource } from "../data/ViewDataSource";
import type { ColumnMeta } from "../ipc/gen/ColumnMeta";
import type { SortKey } from "../ipc/gen/SortKey";
import { SelectionModel } from "../state/selection";
import type { TokenMap } from "../ui/tokens/tokens";
import { cellTooltip, gridCell } from "./cells";
import { copySelection } from "./copy";
import "./grid.css";

export interface ColumnState { key: string; width: number; visible: boolean; pinned: boolean; sort?: "asc" | "desc" }
export interface GridTheme { tokens: TokenMap; fontFamily: string; fontSize: number; zebra: boolean }
export interface DataGridProps {
  source: ViewDataSource;
  columns: ColumnState[];
  selection: SelectionModel;
  onSelectionChange: (selection: SelectionModel) => void;
  onSortChange: (sort: SortKey[]) => void;
  onColumnsChange: (columns: ColumnState[]) => void;
  onContextMenu: (target: { column: string; row?: number; urlId?: number; x: number; y: number }) => void;
  onActivate: (urlId: number, column: string) => void;
  onError: (error: unknown) => void;
  confirmLargeCopy: (rows: number) => Promise<boolean>;
  onExportSelection: () => void;
  rowHeight: number;
  theme: GridTheme;
  ariaLabel: string;
  locale: string;
  /** Measurement/navigation API; never exposes Glide types to consumers. */
  controller?: (controller: GridController | null) => void;
}
export interface GridRect { x: number; y: number; width: number; height: number }
export interface GridController {
  scrollTo: (row: number, column?: number) => void;
  bounds: (column: number, row: number) => GridRect | undefined;
  visible: () => GridRect;
}
export function defaultColumns(columns: ColumnMeta[]): ColumnState[] {
  return columns.map(c => ({ key: c.key, width: c.defaultWidth, visible: c.defaultVisible, pinned: c.key === "Row" || c.key === "Address" }));
}

export function DataGrid(props: DataGridProps) {
  const { source, selection, theme, rowHeight } = props;
  const editor = useRef<DataEditorRef>(null);
  const region = useRef<Rectangle>({ x: 0, y: 0, width: 1, height: 1 });
  const latest = useRef(props); latest.current = props;
  const [revision, refresh] = useState(0);
  const [focus, setFocus] = useState<readonly [number, number]>();
  const [tooltip, setTooltip] = useState<{ text: string; x: number; y: number }>();
  const selectionRequest = useRef(0);
  const visible = useMemo(() => props.columns.filter(c => c.visible || c.key === "Row").sort((a, b) =>
    a.key === "Row" ? -1 : b.key === "Row" ? 1 : Number(b.pinned) - Number(a.pinned)), [props.columns]);
  const metadata = useMemo(() => new Map(source.columns.map((c, i) => [c.key, { column: c, index: i }])), [source]);
  const columns = useMemo(() => visible.map(c => ({ id: c.key, title: c.key, width: c.width, hasMenu: true })), [visible]);
  const freeze = visible.findIndex(c => !c.pinned && c.key !== "Row");
  const tokens = theme.tokens;
  const canvasTheme: Partial<Theme> = { bgCell: tokens["bg.surface"], bgCellMedium: tokens["bg.zebra"], bgHeader: tokens["bg.app"],
    bgHeaderHovered: tokens["bg.hover"], bgHeaderHasFocus: tokens["bg.selection"], textDark: tokens.text, textMedium: tokens["text.muted"],
    textLight: tokens["text.disabled"], textHeader: tokens.text, borderColor: tokens.border, accentColor: tokens.accent,
    accentLight: tokens["bg.selection"], accentFg: tokens["text.onAccent"], fontFamily: theme.fontFamily,
    baseFontStyle: `${theme.fontSize}px`, headerFontStyle: `600 ${theme.fontSize}px` };
  useEffect(() => source.subscribe(() => refresh(n => n + 1)), [source]);
  useEffect(() => {
    source.onError = error => latest.current.onError(error);
    source.onScrollTo = row => editor.current?.scrollTo(0, row, "vertical", 0, 0, { vAlign: "start" });
    return () => { source.onError = undefined; source.onScrollTo = undefined; selectionRequest.current++; };
  }, [source]);
  useEffect(() => {
    const current = selection.forVersion(source.viewId, source.version);
    if (current !== selection) props.onSelectionChange(current);
  }, [selection, source, source.version, props.onSelectionChange]);
  useEffect(() => {
    props.controller?.({ scrollTo: (row, column = 0) => editor.current?.scrollTo(column, row),
      bounds: (column, row) => editor.current?.getBounds(column, row), visible: () => region.current });
    return () => props.controller?.(null);
  }, [props.controller]);
  useEffect(() => {
    let active = true;
    if (selection.focus !== undefined) void source.locate(selection.focus).then(row => {
      if (active && row !== null) setFocus(old => [old?.[0] ?? 0, row]);
    }).catch(error => latest.current.onError(error));
    return () => { active = false; };
  }, [source, source.version, selection.focus]);
  const content = useCallback(([col, row]: readonly [number, number]) => {
    const meta = metadata.get(visible[col].key)!;
    return gridCell(source.getCell(meta.index, row), meta.column, props.locale, tokens);
  }, [source, metadata, visible, props.locale, tokens, revision]);
  const selectRow = async (row: number, col: number, shift = false, toggle = false) => {
    if (!source.total) return;
    const request = ++selectionRequest.current;
    const version = source.version;
    row = Math.max(0, Math.min(source.total - 1, row));
    await source.ensureRow(row);
    const id = source.rowKey(row);
    if (id === undefined || request !== selectionRequest.current || version !== source.version) return;
    let next = latest.current.selection.select(id, toggle);
    if (shift && latest.current.selection.anchor !== undefined) {
      const anchor = await source.locate(latest.current.selection.anchor!);
      if (anchor !== null) next = latest.current.selection.range(await source.keys(Math.min(anchor, row), Math.max(anchor, row)), id);
    }
    if (request !== selectionRequest.current || version !== source.version) return;
    source.setFocus(id, row); setFocus([col, row]);
    latest.current.onSelectionChange(next);
    editor.current?.scrollTo(col, row);
  };
  let selectedRows = CompactSelection.empty();
  for (let row = region.current.y; row < Math.min(source.total, region.current.y + Math.max(100, region.current.height)); row++) {
    const id = source.rowKey(row);
    if (id !== undefined && selection.has(id)) selectedRows = selectedRows.add(row);
  }
  const gridSelection: GridSelection = { columns: CompactSelection.empty(), rows: selectedRows,
    ...(focus ? { current: { cell: focus, range: { x: focus[0], y: focus[1], width: 1, height: 1 }, rangeStack: [] } } : {}) };
  return <div className="os-data-grid" aria-label={props.ariaLabel} onKeyDownCapture={event => {
    const mod = event.metaKey || event.ctrlKey;
    const key = event.key.toLowerCase();
    if (mod && (key === "c" || key === "a")) {
      event.preventDefault(); event.stopPropagation();
      if (key === "a") props.onSelectionChange(selection.all(source.viewId, source.version));
      else void copySelection(source, selection, visible.map(c => c.key), { confirmLarge: props.confirmLargeCopy, offerExport: props.onExportSelection }).catch(props.onError);
    } else if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      const row = focus?.[1] ?? region.current.y;
      const target = event.key === "Home" ? 0 : event.key === "End" ? source.total - 1 : row +
        (event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : event.key === "PageUp" ? -region.current.height : region.current.height);
      void selectRow(target, focus?.[0] ?? 0, event.shiftKey).catch(props.onError);
    }
  }}>
    <DataEditor ref={editor} width="100%" height="100%" columns={columns} rows={source.total}
      rowHeight={rowHeight} headerHeight={rowHeight + 4} freezeColumns={freeze < 0 ? visible.length : freeze}
      rowMarkers="none" theme={canvasTheme} getCellContent={content} gridSelection={gridSelection}
      onGridSelectionChange={() => undefined} onPaste={false}
      rangeSelect="none" rowSelect="none" columnSelect="none"
      getCellsForSelection={range => Array.from({ length: Math.min(500, range.height) }, (_, row) =>
        Array.from({ length: Math.min(visible.length, range.width) }, (_, col) => content([range.x + col, range.y + row])))}
      keybindings={{ copy: false, cut: false, paste: false, selectAll: false, delete: false, downFill: false, rightFill: false }}
      onVisibleRegionChanged={range => { region.current = range; void source.ensureRange(range.y, range.y + range.height - 1); }}
      getRowThemeOverride={row => ({ bgCell: source.rowKey(row) !== undefined && selection.has(source.rowKey(row)!) ? tokens["bg.selection"] : theme.zebra && row % 2 ? tokens["bg.zebra"] : tokens["bg.surface"] })}
      onCellClicked={([col, row], event) => { event.preventDefault(); void selectRow(row, col, event.shiftKey, event.ctrlKey || event.metaKey).catch(props.onError); }}
      onCellActivated={([col, row]) => { const id = source.rowKey(row); if (id !== undefined) props.onActivate(id, visible[col].key); }}
      onColumnResize={(_column, width, index) => props.onColumnsChange(props.columns.map(c => c.key === visible[index].key ? { ...c, width } : c))}
      onColumnMoved={(from, to) => {
        if (visible[from].key === "Row" || visible[to].key === "Row") return;
        const next = [...props.columns];
        if (visible[from].pinned !== visible[to].pinned) return;
        const target = next.findIndex(c => c.key === visible[to].key);
        const index = next.findIndex(c => c.key === visible[from].key);
        const [column] = next.splice(index, 1);
        next.splice(target, 0, column);
        props.onColumnsChange(next);
      }}
      onHeaderClicked={(col, event) => { event.preventDefault(); const c = visible[col]; if (metadata.get(c.key)?.column.sortable) props.onSortChange([{ column: c.key, dir: c.sort === "asc" ? "desc" : "asc" }]); }}
      onCellContextMenu={([col, row], event) => { event.preventDefault(); props.onContextMenu({ column: visible[col].key, row, urlId: source.rowKey(row), x: event.bounds.x, y: event.bounds.y }); }}
      onHeaderContextMenu={(col, event) => { event.preventDefault(); props.onContextMenu({ column: visible[col].key, x: event.bounds.x, y: event.bounds.y }); }}
      onHeaderMenuClick={(col, bounds) => props.onContextMenu({ column: visible[col].key, x: bounds.x, y: bounds.y + bounds.height })}
      onItemHovered={event => {
        if (event.kind !== "cell") { setTooltip(undefined); return; }
        const [col, row] = event.location; const meta = metadata.get(visible[col].key)!;
        const value = source.getCell(meta.index, row);
        setTooltip(value === LOADING ? undefined : { text: cellTooltip(value, meta.column, props.locale), x: event.bounds.x, y: event.bounds.y + event.bounds.height });
      }} />
    {tooltip?.text && <div role="tooltip" className="os-grid-tooltip" style={{ left: tooltip.x, top: tooltip.y }}>{tooltip.text}</div>}
  </div>;
}
