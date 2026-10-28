import { cellBadge, gridHeaderHeight } from "./theme";
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { LOADING, type ViewDataSource } from "../data/ViewDataSource";
import type { ColumnMeta } from "../ipc/gen/ColumnMeta";
import type { SortKey } from "../ipc/gen/SortKey";
import { SelectionModel } from "../state/selection";
import { GRID_DECORATION as D, type TokenMap } from "../ui/tokens/tokens";
import { cellTooltip, ellipsis, gridCell } from "./cells";
import { copySelection } from "./copy";
import { clamp, columnLayout, rowToScroll, rowWindow, scrollExtent, scrollToRow } from "./geometry";
import { DisplayTextMetrics } from "./textMetrics";
import "./grid.css";

export interface ColumnState { key: string; width: number; visible: boolean; pinned: boolean; sort?: "asc" | "desc" }
export interface GridTheme { tokens: TokenMap; fontFamily: string; fontSize: number; zebra: boolean }
export interface DataGridProps {
  /** Optional upper-table sorting gesture and title decoration (M2-U10). */
  onSortGesture?: (column: string, extend: boolean) => void;
  headerTitle?: (column: string) => string;
  /** Optional label colours per cell, e.g. the segment column (M2-U20, UI-020); additive, default none. */
  cellColors?: (column: string, value: unknown) => { background: string; text: string } | undefined;
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
  /** Optional feature-specific titles and empty-cell explanations; canonical keys remain unchanged. */
  columnTitles?: Readonly<Record<string, string>>;
  columnHints?: Readonly<Record<string, string>>;
  ariaLabel: string;
  locale: string;
  /** Measurement/navigation API; never exposes renderer types to consumers. */
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
  const root = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const measuring = useRef<HTMLSpanElement>(null);
  const metrics = useRef<DisplayTextMetrics | null>(null);
  const latest = useRef(props); latest.current = props;
  const id = useId();
  const [, refresh] = useState(0);
  const [size, setSize] = useState({ width: 0, height: 0, scale: 1, dpr: 1, overlayX: 0 });
  const [position, setPosition] = useState({ row: 0, left: 0 });
  const [focus, setFocus] = useState({ col: 0, row: 0 });
  const [tooltip, setTooltip] = useState<{ text: string; x: number; y: number }>();
  const selectionRequest = useRef(0);
  const expectedTop = useRef<number | undefined>(undefined);
  const geometryChanging = useRef(false);
  const wheelRemainder = useRef(0);
  const gesture = useRef<{ col: number; start: number; width: number; resize: boolean; moved: boolean } | undefined>(undefined);
  const visible = useMemo(() => props.columns.filter(c => c.visible || c.key === "Row").sort((a, b) =>
    a.key === "Row" ? -1 : b.key === "Row" ? 1 : Number(b.pinned) - Number(a.pinned)), [props.columns]);
  const markerColumn = visible.some(c => c.key === "Row") ? "Row" : "Address";
  const metadata = useMemo(() => new Map(source.columns.map((c, index) => [c.key, { column: c, index }])), [source]);
  const { capacity, maxFirst } = rowWindow(source.total, size.height, rowHeight);
  const first = clamp(position.row, maxFirst);
  const headerHeight = gridHeaderHeight(rowHeight);
  const count = Math.min(source.total - first, Math.max(0, Math.ceil((size.height - headerHeight) / rowHeight)));
  const totalWidth = visible.reduce((n, c) => n + c.width, 0);
  const extent = scrollExtent(source.total, size.height, rowHeight);
  const layout = columnLayout(visible, position.left, size.width);
  const region: GridRect = { x: layout.find(c => !c.pinned && c.key !== "Row")?.index ?? 0, y: first, width: layout.length, height: count };
  const runtime = useRef({ first, capacity, maxFirst, extent, size, layout, region });
  runtime.current = { first, capacity, maxFirst, extent, size, layout, region };

  // Keep exact logical rows separate from the compressed native scrollbar. Its rounded
  // scroll event must never replace a requested row (5M rows exceed CSS pixel precision).
  const move = (row: number, left = position.left, limit = maxFirst) => {
    row = Math.round(clamp(row, limit));
    const element = scroller.current;
    if (element) {
      element.scrollTop = rowToScroll(row, limit, element.scrollHeight - element.clientHeight);
      expectedTop.current = element.scrollTop;
      element.scrollLeft = left;
      left = element.scrollLeft;
    }
    setPosition({ row, left });
    setTooltip(undefined);
  };
  const reveal = (row: number, col: number, alignStart = false) => {
    let left = position.left;
    const column = visible[col];
    if (column && !column.pinned && column.key !== "Row") {
      const x = visible.slice(0, col).reduce((n, c) => n + c.width, 0);
      const pinned = visible.filter(c => c.pinned || c.key === "Row").reduce((n, c) => n + c.width, 0);
      if (x - left < pinned) left = x - pinned;
      else if (x + column.width - left > size.width) left = x + column.width - size.width;
    }
    move(alignStart ? row : row < first ? row : row >= first + capacity ? row - capacity + 1 : first, left);
  };
  const bounds = (col: number, row: number): GridRect | undefined => {
    const c = visible[col];
    if (!c || row < -1 || row >= source.total) return undefined;
    // The controller reports geometry even for virtualized columns/rows; it does
    // not imply that the cell is painted or hit-testable inside the viewport.
    const x = visible.slice(0, col).reduce((n, column) => n + column.width, 0) - (c.pinned || c.key === "Row" ? 0 : position.left);
    const rect = canvas.current?.getBoundingClientRect();
    if (!rect) return undefined;
    return { x: rect.x + x * size.scale, y: rect.y + (row === -1 ? 0 : headerHeight + (row - first) * rowHeight) * size.scale,
      width: c.width * size.scale, height: (row === -1 ? headerHeight : rowHeight) * size.scale };
  };
  const actions = useRef({ move, reveal, bounds }); actions.current = { move, reveal, bounds };
  useLayoutEffect(() => {
    const measure = () => {
      const element = scroller.current!;
      const rect = element.getBoundingClientRect();
      const scale = element.offsetWidth ? rect.width / element.offsetWidth : 1;
      const overlayX = Math.max(0, 14 - (element.offsetWidth - element.clientWidth));
      const overlayY = Math.max(0, 14 - (element.offsetHeight - element.clientHeight));
      const next = { width: Math.max(0, element.clientWidth - overlayX), height: Math.max(0, element.clientHeight - overlayY), scale, dpr: window.devicePixelRatio || 1, overlayX };
      // WebKit dispatches a rounded scroll event between CSS zoom's mutation and
      // React's resize commit. Preserve the logical anchor across that event.
      if (Object.keys(next).some(key => runtime.current.size[key as keyof typeof next] !== next[key as keyof typeof next])) geometryChanging.current = true;
      setSize(old => Object.keys(next).every(key => old[key as keyof typeof old] === next[key as keyof typeof next]) ? old : next);
    };
    const resize = new ResizeObserver(measure); resize.observe(root.current!); resize.observe(scroller.current!);
    const mutations = new MutationObserver(measure);
    for (let node: HTMLElement | null = root.current; node; node = node.parentElement) mutations.observe(node, { attributes: true, attributeFilter: ["style", "class"] });
    let resolution: MediaQueryList;
    const dprChanged = () => { resolution?.removeEventListener("change", dprChanged); measure(); resolution = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`); resolution.addEventListener("change", dprChanged); };
    dprChanged(); window.addEventListener("resize", measure);
    return () => { resize.disconnect(); mutations.disconnect(); resolution.removeEventListener("change", dprChanged); window.removeEventListener("resize", measure); };
  }, []);
  useLayoutEffect(() => { actions.current.move(runtime.current.first, scroller.current?.scrollLeft ?? 0); geometryChanging.current = false; }, [size, rowHeight, source.total, totalWidth]);
  useEffect(() => {
    let frame = 0;
    const unsubscribe = source.subscribe(() => { if (!frame) frame = requestAnimationFrame(() => { frame = 0; refresh(n => n + 1); }); });
    source.onError = error => latest.current.onError(error);
    // A new version brings a new total before the next render: clamp against the source's current total, not the rendered one (A01-10).
    source.onScrollTo = row => actions.current.move(row, undefined, rowWindow(source.total, runtime.current.size.height, latest.current.rowHeight).maxFirst);
    return () => { unsubscribe(); cancelAnimationFrame(frame); source.onError = undefined; source.onScrollTo = undefined; selectionRequest.current++; };
  }, [source]);
  useEffect(() => { void source.ensureRange(first, first + Math.max(1, count) - 1); }, [source, source.version, first, count]);
  useEffect(() => {
    const current = selection.forVersion(source.viewId, source.version);
    if (current !== selection) props.onSelectionChange(current);
  }, [selection, source, source.version, props.onSelectionChange]);
  useEffect(() => {
    props.controller?.({ scrollTo: (row, col = 0) => actions.current.reveal(row, col, true), bounds: (col, row) => actions.current.bounds(col, row), visible: () => runtime.current.region });
    return () => props.controller?.(null);
  }, [props.controller]);
  useEffect(() => {
    let active = true;
    if (selection.focus !== undefined) void source.locate(selection.focus).then(row => {
      if (active && row !== null) setFocus(old => ({ col: old.col, row }));
    }).catch(error => latest.current.onError(error));
    return () => { active = false; };
  }, [source, source.version, selection.focus]);
  useEffect(() => {
    const element = canvas.current!;
    const wheel = (event: WheelEvent) => {
      if (event.ctrlKey) return; // Browser/trackpad zoom remains available.
      event.preventDefault();
      const r = runtime.current;
      const factor = event.deltaMode === 1 ? latest.current.rowHeight : event.deltaMode === 2 ? r.size.height : 1 / r.size.scale;
      const horizontal = event.shiftKey && !event.deltaX ? event.deltaY : event.deltaX;
      wheelRemainder.current += (event.shiftKey ? 0 : event.deltaY) * factor;
      const rows = Math.trunc(wheelRemainder.current / latest.current.rowHeight);
      wheelRemainder.current -= rows * latest.current.rowHeight;
      actions.current.move(r.first + rows, (scroller.current?.scrollLeft ?? 0) + horizontal * factor);
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => element.removeEventListener("wheel", wheel);
  }, []);
  const cell = (col: number, row: number) => {
    const meta = metadata.get(visible[col].key);
    if (!meta) return { text: "", color: theme.tokens.text, align: "left" as const, loading: false };
    const raw = source.getCell(meta.index, row);
    const cell = gridCell(raw, meta.column, props.locale, theme.tokens);
    const colors = raw === LOADING ? undefined : props.cellColors?.(meta.column.key, raw);
    return colors ? { ...cell, color: colors.text, background: colors.background } : cell;
  };
  useLayoutEffect(() => {
    const element = canvas.current!;
    const rect = element.getBoundingClientRect();
    const width = Math.round(rect.width * size.dpr), height = Math.round(rect.height * size.dpr);
    if (element.width !== width) element.width = width;
    if (element.height !== height) element.height = height;
    const ctx = element.getContext("2d");
    if (!ctx || !size.width || !size.height) return;
    ctx.setTransform(width / size.width, 0, 0, height / size.height, 0, 0);
    const t = theme.tokens;
    ctx.fillStyle = t["bg.surface"]; ctx.fillRect(0, 0, size.width, size.height);
    ctx.textBaseline = "middle";
    metrics.current ??= new DisplayTextMetrics(measuring.current!);
    const measure = (text: string) => metrics.current!.width(text);
    for (let row = first - 1; row < first + count; row++) {
      const header = row === first - 1;
      const y = header ? 0 : headerHeight + (row - first) * rowHeight;
      const h = header ? headerHeight : rowHeight;
      const key = source.rowKey(row);
      ctx.fillStyle = header ? t["bg.app"] : key !== undefined && selection.has(key) ? t["bg.selection"] : theme.zebra && row % 2 ? t["bg.zebra"] : t["bg.surface"];
      ctx.fillRect(0, y, size.width, h);
      ctx.font = `${theme.fontSize}px ${theme.fontFamily}`;
      metrics.current.setFont(ctx.font);
      for (const c of layout) {
        ctx.save(); ctx.beginPath(); ctx.rect(c.clipLeft, y, c.clipRight - c.clipLeft, h); ctx.clip();
        const value = header ? { text: props.headerTitle?.(c.key) ?? (props.columnTitles?.[c.key] ?? c.key) + (c.sort ? ` ${c.sort === "asc" ? "↑" : "↓"} ${visible.filter(c => c.sort).findIndex(s => s.key === c.key) + 1}` : ""), color: t.text, align: "left" as const } : cell(c.index, row);
        const background = (value as { background?: string }).background;
        if (background) { ctx.fillStyle = background; ctx.fillRect(c.x, y, c.width, h); }
        const badge = header || background ? null : cellBadge(c.key, value.text, t);
        if (badge) {
          const badgeWidth = Math.min(c.width - D.badgeInset * 2, measure(value.text) + D.badgePadding * 2);
          const badgeX = c.x + (value.align === "right" ? c.width - badgeWidth - D.badgeInset : value.align === "center" ? (c.width - badgeWidth) / 2 : D.badgeInset);
          ctx.fillStyle = badge.background;
          ctx.beginPath(); ctx.roundRect(badgeX, y + (h - D.badgeHeight) / 2, badgeWidth, D.badgeHeight, D.badgeRadius); ctx.fill();
        }
        if (!header && c.key === markerColumn) {
          ctx.strokeStyle = key !== undefined && selection.has(key) ? t.accent : t["border.strong"];
          ctx.lineWidth = 1; ctx.beginPath(); ctx.roundRect(c.x + D.checkboxInset, y + (h - D.checkboxSize) / 2, D.checkboxSize, D.checkboxSize, D.checkboxRadius); ctx.stroke();
          if (key !== undefined && selection.has(key)) { ctx.fillStyle = t.accent; ctx.fillRect(c.x + D.checkboxInset + D.checkboxSize / 4, y + h / 2 - D.checkboxSize / 4, D.checkboxSize / 2, D.checkboxSize / 2); }
        }
        ctx.fillStyle = badge?.text ?? value.color; ctx.textAlign = value.align;
        const x = c.x + (value.align === "right" ? c.width - 8 : value.align === "center" ? c.width / 2 : c.key === "Address" && markerColumn === "Address" ? D.addressPadding : D.labelInset);
        ctx.fillText(ellipsis(measure, value.text, c.width - (c.key === "Address" ? (markerColumn === "Address" ? D.addressReserved : D.labelInset + D.addressAction) : header ? 30 : 16)), x, y + h / 2);
        if (!header && c.key === "Address") { ctx.textAlign = "right"; ctx.fillStyle = t["text.muted"]; ctx.fillText("⋮", c.x + c.width - D.labelInset, y + h / 2); }
        if (header) { ctx.textAlign = "right"; ctx.fillText("⋮", c.x + c.width - 8, y + h / 2); }
        ctx.strokeStyle = t.border; ctx.lineWidth = 1; ctx.strokeRect(c.x - 0.5, y - 0.5, c.width, h);
        if (focus.col === c.index && focus.row === (header ? -1 : row)) { ctx.strokeStyle = t.accent; ctx.lineWidth = 2; ctx.strokeRect(c.x + 1, y + 1, c.width - 2, h - 2); }
        ctx.restore();
      }
    }
  }); // Paint only on a committed change; no continuous animation loop.
  const selectRow = async (row: number, col: number, shift = false, toggle = false) => {
    if (!source.total) return;
    const request = ++selectionRequest.current, version = source.version;
    row = clamp(row, source.total - 1);
    // A click targets the row on screen: resolve its URL id synchronously when the block is loaded, so a view update that
    // lands meanwhile cannot swallow the click (A01-16). Only a block that still has to be fetched is bound to its version.
    let key = source.rowKey(row);
    if (key === undefined) {
      await source.ensureRow(row);
      if (version !== source.version) return;
      key = source.rowKey(row);
    }
    if (key === undefined || request !== selectionRequest.current) return;
    let next = latest.current.selection.select(key, toggle);
    if (shift && latest.current.selection.anchor !== undefined) {
      const anchor = await source.locate(latest.current.selection.anchor!);
      if (anchor !== null) {
        const firstSelected = Math.min(anchor, row), lastSelected = Math.max(anchor, row);
        next = firstSelected === 0 && lastSelected === source.total - 1
          ? latest.current.selection.range([], key).all(source.viewId, version)
          : latest.current.selection.range(await source.keys(firstSelected, lastSelected), key);
      }
    }
    if (request !== selectionRequest.current || (shift && version !== source.version)) return;
    source.setFocus(key, row); setFocus({ col, row }); latest.current.onSelectionChange(next);
    actions.current.reveal(row, col);
  };
  const sort = (col: number, multi: boolean) => {
    const c = visible[col];
    if (metadata.get(c.key)?.column.sortable && props.onSortGesture) { props.onSortGesture(c.key, multi); return; }
    if (metadata.get(c.key)?.column.sortable) props.onSortChange([
      ...(multi ? visible.filter(s => s.sort && s.key !== c.key).map(s => ({ column: s.key, dir: s.sort! })) : []),
      { column: c.key, dir: c.sort === "asc" ? "desc" : "asc" },
    ]);
  };
  const contextMenu = (col: number, row: number, x: number, y: number) => {
    props.onContextMenu({ column: visible[col].key, ...(row >= 0 ? { row, urlId: source.rowKey(row) } : {}), x, y });
  };
  const hit = (x: number, y: number) => {
    const rect = canvas.current!.getBoundingClientRect();
    x = (x - rect.x) / size.scale; y = (y - rect.y) / size.scale;
    const column = layout.find(c => x >= c.clipLeft && x < c.clipRight);
    const row = y < headerHeight ? -1 : first + Math.floor((y - headerHeight) / rowHeight);
    return column && row < source.total && y >= 0 && y < size.height ? { column, row, x } : undefined;
  };
  const resizeColumn = (col: number, width: number) => props.onColumnsChange(props.columns.map(c => c.key === visible[col].key ? { ...c, width: Math.max(40, Math.round(width)) } : c));
  const activeRow = focus.row === -1 ? -1 : clamp(focus.row, Math.max(0, source.total - 1));
  const activeCol = clamp(focus.col, Math.max(0, visible.length - 1));
  const activeVisible = layout.some(c => c.index === activeCol) && (activeRow === -1 || (activeRow >= first && activeRow < first + count));
  const cellId = (col: number, row: number) => `${id}-${col}-${row}`;
  return <div ref={root} className="os-data-grid" role="grid" tabIndex={0} aria-label={props.ariaLabel}
    aria-rowcount={source.total + 1} aria-colcount={visible.length} aria-multiselectable="true" aria-readonly="true"
    aria-activedescendant={activeVisible ? cellId(activeCol, activeRow) : undefined}
    onKeyDown={event => {
      if (event.nativeEvent.isComposing || !visible.length) return;
      const mod = event.metaKey || event.ctrlKey, key = event.key.toLowerCase();
      if (mod && (key === "c" || key === "a")) {
        event.preventDefault(); event.stopPropagation();
        if (key === "a") { selectionRequest.current++; props.onSelectionChange(selection.all(source.viewId, source.version)); }
        else void copySelection(source, selection, visible.map(c => c.key), { confirmLarge: props.confirmLargeCopy, offerExport: props.onExportSelection }).catch(props.onError);
      } else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
        event.preventDefault(); const b = bounds(activeCol, activeRow); if (b) contextMenu(activeCol, activeRow, b.x, b.y + b.height);
      } else if (event.key === "Enter") {
        event.preventDefault();
        if (activeRow === -1) sort(activeCol, event.shiftKey);
        else { const key = source.rowKey(activeRow); if (key !== undefined) props.onActivate(key, visible[activeCol].key); }
      } else if (["ArrowLeft", "ArrowRight"].includes(event.key)) {
        event.preventDefault(); const col = clamp(activeCol + (event.key === "ArrowLeft" ? -1 : 1), visible.length - 1);
        setFocus({ col, row: activeRow }); reveal(Math.max(first, activeRow), col);
      } else if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(event.key)) {
        event.preventDefault(); event.stopPropagation();
        if (event.key === "ArrowUp" && activeRow === 0 && !event.shiftKey) { setFocus({ col: activeCol, row: -1 }); return; }
        const row = event.key === "Home" ? 0 : event.key === "End" ? source.total - 1 : activeRow +
          (event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : event.key === "PageUp" ? -capacity : capacity);
        void selectRow(row, activeCol, event.shiftKey).catch(props.onError);
      } else if (event.key === " ") {
        event.preventDefault(); if (activeRow === -1) sort(activeCol, event.shiftKey); else void selectRow(activeRow, activeCol, event.shiftKey, mod).catch(props.onError);
      } else if (event.key === "Escape") { selectionRequest.current++; setTooltip(undefined); }
    }}>
    <div ref={scroller} className="os-grid-scroll" aria-hidden="true" onScroll={event => {
      const e = event.currentTarget;
      if (geometryChanging.current) return;
      const same = expectedTop.current !== undefined && Math.abs(e.scrollTop - expectedTop.current) < 1;
      if (!same) expectedTop.current = undefined;
      setPosition(old => ({ row: same ? old.row : scrollToRow(e.scrollTop, maxFirst, e.scrollHeight - e.clientHeight), left: e.scrollLeft }));
      setTooltip(undefined);
    }}><div style={{ width: totalWidth + size.overlayX, height: extent }} /></div>
    <canvas ref={canvas} className="os-grid-canvas" aria-hidden="true" style={{ width: size.width, height: size.height }}
      onPointerDown={event => {
        if (event.button !== 0) return;
        root.current?.focus({ preventScroll: true }); event.preventDefault();
        const h = hit(event.clientX, event.clientY); if (!h) return;
        setFocus({ col: h.column.index, row: h.row });
        if (h.row === -1) {
          gesture.current = { col: h.column.index, start: event.clientX, width: h.column.width, resize: h.column.x + h.column.width - h.x < 6, moved: false };
          event.currentTarget.setPointerCapture(event.pointerId);
        } else if (h.column.key === "Address" && h.column.x + h.column.width - h.x < D.addressAction) contextMenu(h.column.index, h.row, event.clientX, event.clientY);
        else void selectRow(h.row, h.column.index, event.shiftKey, event.ctrlKey || event.metaKey || (h.column.key === markerColumn && h.x - h.column.x < D.addressAction)).catch(props.onError);
      }}
      onPointerMove={event => {
        const g = gesture.current;
        if (g) {
          if (Math.abs(event.clientX - g.start) > 4) g.moved = true;
          if (g.resize) resizeColumn(g.col, g.width + (event.clientX - g.start) / size.scale);
          return;
        }
        const h = hit(event.clientX, event.clientY); const meta = h && metadata.get(h.column.key);
        const value = h && meta && h.row >= 0 ? source.getCell(meta.index, h.row) : LOADING;
        const text = value === LOADING || !meta ? "" : cellTooltip(value, meta.column, props.locale) || props.columnHints?.[meta.column.key] || "";
        setTooltip(old => old?.text === text ? old : text ? { text, x: event.clientX, y: event.clientY + 18 } : undefined);
        event.currentTarget.style.cursor = h?.row === -1 && h.column.x + h.column.width - h.x < 6 ? "col-resize" : "default";
      }}
      onPointerUp={event => {
        const g = gesture.current; gesture.current = undefined;
        if (!g) return;
        event.currentTarget.releasePointerCapture(event.pointerId);
        const h = hit(event.clientX, event.clientY); if (!h || g.resize) return;
        if (g.moved) {
          const from = visible[g.col], to = h.column;
          if (from.key === "Row" || to.key === "Row" || from.pinned !== to.pinned) return;
          const next = [...props.columns], target = next.findIndex(c => c.key === to.key);
          const [column] = next.splice(next.findIndex(c => c.key === from.key), 1); next.splice(target, 0, column);
          props.onColumnsChange(next);
        } else if (h.column.x + h.column.width - h.x < 26) contextMenu(g.col, -1, event.clientX, event.clientY);
        else sort(g.col, event.shiftKey);
      }}
      onPointerCancel={() => { gesture.current = undefined; }}
      onPointerLeave={() => setTooltip(undefined)}
      onDoubleClick={event => {
        const h = hit(event.clientX, event.clientY); if (!h) return;
        if (h.row === -1 && h.column.x + h.column.width - h.x < 6) {
          const ctx = canvas.current!.getContext("2d"); if (!ctx) return;
          ctx.font = `${theme.fontSize}px ${theme.fontFamily}`;
          metrics.current ??= new DisplayTextMetrics(measuring.current!);
          metrics.current.setFont(ctx.font);
          let width = metrics.current.width(h.column.key) + 40;
          for (let row = first; row < first + count; row++) width = Math.max(width, metrics.current.width(cell(h.column.index, row).text) + 16);
          resizeColumn(h.column.index, Math.min(800, width));
        } else if (h.row >= 0 && !(h.column.key === markerColumn && h.x - h.column.x < D.addressAction) && !(h.column.key === "Address" && h.column.x + h.column.width - h.x < D.addressAction)) { const key = source.rowKey(h.row); if (key !== undefined) props.onActivate(key, h.column.key); }
      }}
      onContextMenu={event => { event.preventDefault(); const h = hit(event.clientX, event.clientY); if (h) contextMenu(h.column.index, h.row, event.clientX, event.clientY); }} />
    <span ref={measuring} className="os-grid-measure" aria-hidden="true" />
    <div className="os-grid-a11y">
      <div role="row" aria-rowindex={1}>{layout.map(c => <div role="columnheader" id={cellId(c.index, -1)} key={c.key} aria-colindex={c.index + 1}
        aria-sort={c.sort === "asc" ? "ascending" : c.sort === "desc" ? "descending" : undefined}>{props.headerTitle?.(c.key) ?? props.columnTitles?.[c.key] ?? c.key}</div>)}</div>
      {Array.from({ length: Math.max(0, count) }, (_, index) => {
        const row = first + index, key = source.rowKey(row);
        return <div role="row" aria-rowindex={row + 2} aria-busy={key === undefined} aria-selected={key !== undefined && selection.has(key)} key={row}>
          {layout.map(c => <div role="gridcell" id={cellId(c.index, row)} key={c.key} aria-colindex={c.index + 1}>{cell(c.index, row).text}</div>)}
        </div>;
      })}
    </div>
    {tooltip?.text && <div role="tooltip" className="os-grid-tooltip" style={{ left: tooltip.x, top: tooltip.y }}>{tooltip.text}</div>}
  </div>;
}
