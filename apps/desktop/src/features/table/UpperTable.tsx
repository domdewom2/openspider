import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { DataGrid, type GridController } from "../../grid/DataGrid";
import { copySelection } from "../../grid/copy";
import { currentLanguage, useT } from "../../i18n";
import { crawl } from "../../ipc/crawl";
import { eventBus } from "../../ipc/events";
import { toCmdError } from "../../ipc/errors";
import { exportApi } from "../../ipc/export";
import { op } from "../../ipc/op";
import type { FilterInfo } from "../../ipc/gen/FilterInfo";
import type { ViewSpec } from "../../ipc/gen/ViewSpec";
import { useColumns, exportColumns } from "../../state/columns";
import { usePrefs } from "../../state/prefs";
import { useSession } from "../../state/session";
import { cycleSort, defaultQuery, useTabs } from "../../state/tabs";
import { useAction } from "../../ui/actions";
import { currentPlatform } from "../../ui/platform";
import { Button } from "../../ui/primitives/Button";
import { Dialog, DialogFooter } from "../../ui/primitives/Dialog";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { MenuBody, type MenuEntry } from "../../ui/primitives/Menu";
import { Popover } from "react-aria-components";
import { FONT_STACKS, SIZES, resolveTokens } from "../../ui/tokens/tokens";
import { resolveTheme, systemPrefersDark } from "../../ui/tokens/theme";
import { ColumnsMenu } from "./ColumnsMenu";
import { confirmsRemoval } from "./RemovalConfirmationSettings";
import { saveTableState } from "../../state/tabs";
import { TableController } from "./controller";
import { contextSelection, openSelected, launchBrowser } from "./actions";
import "./table.css";

export function filterLabel(filter: FilterInfo, t: ReturnType<typeof useT>, locale: string): string {
  const number = filter.state.kind === "needsCrawlAnalysis" || filter.count == null ? "–" : new Intl.NumberFormat(locale).format(filter.count);
  const issue = filter.issue ? `${filter.issue.issueType === "issue" ? "●" : filter.issue.issueType === "warning" ? "▲" : "◆"} ${t(`u10-issue-${filter.issue.issueType}`)} · ${t(`u10-priority-${filter.issue.priority}`)} · ` : "";
  const state = filter.state.kind === "ready" ? "" : ` · ${t(`u10-filter-${filter.state.kind}`)}`;
  return `${issue}${t(`filter-${filter.labelTemplate.toLowerCase().replaceAll(" ", "-")}`, filter.params, { fallback: filter.id })} (${number})${state}`;
}
export function UpperTable() {
  const t = useT(); const locale = currentLanguage(); const tabs = useTabs();
  const query = tabs.queries[tabs.active] ?? defaultQuery();
  const appearance = usePrefs(s => s.appearance);
  const crawlId = useSession(s => s.crawl?.crawlId);
  const columnState = useColumns();
  const columns = columnState.byTab[tabs.active] ?? [];
  const controller = useMemo(() => new TableController(), []);
  useSyncExternalStore(controller.subscribe, controller.snapshot);
  const source = controller.source;
  const grid = useRef<GridController | null>(null);
  const search = useRef<HTMLInputElement>(null); const filterRef = useRef<HTMLSelectElement>(null);
  const [text, setText] = useState(query.search);
  const [context, setContext] = useState<{ column: string; urlId?: number; x: number; y: number }>();
  const anchor = useRef<HTMLSpanElement>(null);
  const [operation, setOperation] = useState<{ id: string; total: number }>();
  const status = useSession(s => operation ? s.ops[operation.id] : undefined);
  const analysis = useSession(s => s.analysis);
  const [stale, setStale] = useState(false);
  const [remove, setRemove] = useState(false); const [skipRemoval, setSkipRemoval] = useState(false);
  const [browserConfirm, setBrowserConfirm] = useState(false);
  const browserResolve = useRef<((ok: boolean) => void) | undefined>(undefined);
  const [exportSpec, setExportSpec] = useState<ViewSpec>();
  const [path, setPath] = useState(""); const [allColumns, setAllColumns] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const count = source ? controller.selection.count(source.total) : 0;
  const current = tabs.catalog.find(tab => tab.id === tabs.active);
  const filters = (current?.filters ?? []).map(f => controller.counts.filters.find(row => row.tab === tabs.active && row.filter.id === f.id)?.filter ?? f)
    .sort((a, b) => a.id === "All" ? -1 : b.id === "All" ? 1 : 0);
  const activeFilter = filters.find(f => f.id === query.filter);
  const total = filters.find(f => f.id === "All")?.count;
  const fail = controller.fail;
  useEffect(() => { controller.activate(); return () => { controller.dispose(); browserResolve.current?.(false); }; }, [controller]);
  useEffect(() => { controller.resetCrawl(); }, [controller, crawlId]);
  useEffect(() => {
    void columnState.load(tabs.active).catch(fail);
  }, [tabs.active, crawlId]);
  useEffect(() => {
    if (!tabs.loaded) return;
    void controller.open(tabs.active, query);
    setText(query.search);
  }, [controller, tabs.loaded, tabs.active, query.filter, query.search, JSON.stringify(query.sort), crawlId]);
  useEffect(() => {
    void controller.refreshCounts();
    return eventBus.on("tick", tick => { void controller.tick(tick).catch(fail); });
  }, [controller, crawlId]);
  const setGrid = useCallback((value: GridController | null) => { grid.current = value; }, []);
  useEffect(() => {
    const id = tabs.focusId ?? controller.selection.focus;
    if (id === undefined || !source) return;
    let live = true;
    void controller.focus(id).then(row => { if (live && row !== null) grid.current?.scrollTo(row); }).catch(fail);
    return () => { live = false; };
  }, [source, tabs.focusRequest]);
  useEffect(() => { if (status?.state === "failed" && status.error) fail(status.error); }, [status]);
  useEffect(() => { if (analysis && !analysis.running && analysis.steps.length && analysis.steps.every(step => !step.stale)) setStale(false); }, [analysis]);
  const busy = operation !== undefined && (!status || status.state === "running");
  const actOnSelection = async (action: "recrawl" | "remove") => {
    if (!source || !count || busy) return;
    const selection = controller.selection.forVersion(source.viewId, source.version);
    const total = selection.count(source.total); if (!total) return;
    const result = await crawl[action]({ selection: selection.value });
    setOperation({ id: result.opId, total }); setStale(true); setRemove(false); setContext(undefined);
  };
  const requestRemoval = async (fromKeyboard = false) => {
    if (!count || busy) return;
    if (!fromKeyboard && !await confirmsRemoval()) await actOnSelection("remove");
    else { setSkipRemoval(false); setRemove(true); }
  };
  const copy = (keys: string[], cell = false) => {
    if (!source) return;
    const selection = cell && context?.urlId !== undefined ? controller.selection.select(context.urlId) : controller.selection;
    void copySelection(source, selection, keys, { confirmLarge: async () => window.confirm(t("u10-large-copy")), offerExport: () => setExportOpen(true), withHeader: false }).catch(fail);
  };
  const openBrowser = () => {
    if (!source) return;
    void openSelected(source, controller.selection, () => new Promise(resolve => { browserResolve.current = resolve; setBrowserConfirm(true); }),
      launchBrowser).catch(fail);
  };
  const openExport = (spec?: ViewSpec) => { setExportSpec(spec); setPath(""); setExportOpen(true); setContext(undefined); };
  const exportSelection = async () => {
    if (!source || !path.trim()) return;
    const result = exportSpec
      ? await exportApi.detail({ spec: exportSpec, selection: controller.selection.value, path, format: "csv" })
      : await exportApi.view({ viewId: source.viewId, version: source.version, path, format: "csv", allColumns, columns: exportColumns(columns) });
    setOperation({ id: result.opId, total: exportSpec ? count : source.total }); setExportOpen(false);
  };
  useAction("search.focus", () => search.current?.focus());
  useAction("search.clear", () => { tabs.query({ search: "" }); setText(""); });
  useAction("filter.open", () => filterRef.current?.focus());
  useAction("export.view", () => openExport());
  useAction("crawl.recrawl", () => { void actOnSelection("recrawl").catch(fail); });
  useAction("table.remove", () => { void requestRemoval(true).catch(fail); });
  useAction("browser.open", openBrowser);
  const showContext = (target: { column: string; urlId?: number; x: number; y: number }) => {
    if (target.urlId !== undefined) controller.setSelection(contextSelection(controller.selection, target.urlId));
    setContext(target);
  };
  useAction("context.menu", () => {
    if (controller.selection.focus === undefined) return false;
    const rect = grid.current?.bounds(1, grid.current.visible().y);
    showContext({ column: "Address", urlId: controller.selection.focus, x: rect?.x ?? 0, y: rect?.y ?? 0 });
  });
  const entries: MenuEntry[] = context && context.urlId === undefined ? [
    { id: "pin", label: t("u10-pin-address"), onAction: () => void columnState.change(tabs.active, columns.map(c => c.key === "Address" ? { ...c, pinned: !c.pinned } : c)).catch(fail) },
  ] : [
    { id: "copy-rows", label: t("u10-copy-rows"), onAction: () => copy(exportColumns(columns)) },
    { id: "copy-url", label: t("u10-copy-url"), onAction: () => copy(["Address"]) },
    { id: "copy-cell", label: t("u10-copy-cell"), onAction: () => copy([context?.column ?? "Address"], true) },
    { id: "copy-column", label: t("u10-copy-column"), onAction: () => copy([context?.column ?? "Address"]) },
    { id: "browser", label: t("u10-browser"), onAction: openBrowser },
    { id: "recrawl", label: t("u10-recrawl"), onAction: () => void actOnSelection("recrawl").catch(fail), isDisabled: busy, disabledReason: busy ? t("u10-busy") : undefined },
    { id: "remove", label: t("u10-remove"), onAction: () => { void requestRemoval().catch(fail); }, isDisabled: busy, disabledReason: busy ? t("u10-busy") : undefined },
    ...(["inlinks", "outlinks", "images", "resources", "crawlPath"] as const).map(kind => ({ id: `export-${kind}`, label: t(`u10-export-${kind}`), onAction: () => {
      const urlIds = controller.selection.value.kind === "ids" ? controller.selection.value.ids : [];
      openExport(kind === "crawlPath" ? { kind: "detail", detail: "crawlPath", urlIds, params: null, sort: [] } :
        kind === "images" ? { kind, urlIds, sort: [] } : { kind, urlIds, filter: { id: "All" }, sort: [] });
    } })),
  ];
  const available = count > 0;
  const menuEntries = entries.map(e => ({ ...e, isDisabled: e.isDisabled || (!available && context?.urlId !== undefined),
    disabledReason: e.disabledReason ?? (!available ? t("u10-no-selection") : undefined), onAction: () => { e.onAction?.(); setContext(undefined); } }));
  const empty = activeFilter?.state.kind === "needsCrawlAnalysis" ? { code: "NEEDS_CRAWL_ANALYSIS" as const } :
    activeFilter?.state.kind === "needsConfig" ? { code: "FEATURE_DISABLED" as const, arg: activeFilter.state.key } :
    activeFilter?.state.kind === "needsRendering" ? { code: "NEEDS_JS_RENDERING" as const } : null;
  return <section className="u10-table" data-shortcut-scope="table" aria-label={t("u10-table")}>
    <div className="u10-toolbar">
      <label>{t("u10-filter")}<select ref={filterRef} data-priority={activeFilter?.issue?.priority} value={query.filter} onChange={e => tabs.query({ filter: e.target.value })}>
        {filters.map(f => <option key={f.id} value={f.id} title={filterLabel(f, t, locale)} data-priority={f.issue?.priority}>{filterLabel(f, t, locale)}</option>)}
      </select></label>
      <form onSubmit={e => { e.preventDefault(); tabs.query({ search: text }); }}>
        <label>{t("u10-search")}<input ref={search} type="search" value={text} onChange={e => setText(e.target.value)}
          onKeyDown={e => { if (e.key === "Escape") { setText(""); tabs.query({ search: "" }); } }} /></label>
      </form>
      <Button onPress={() => openExport()} isDisabled={!source}>{t("u10-export")}</Button>
      <ColumnsMenu columns={columns} onChange={next => void columnState.change(tabs.active, next).catch(fail)} onReset={() => void columnState.reset(tabs.active).catch(fail)} />
    </div>
    {controller.error != null && <div role="alert">{t(toCmdError(controller.error).messageKey, toCmdError(controller.error).args)}</div>}
    {stale && <p role="status">{t("u10-stale")}</p>}
    {operation && <div role="status" className="u10-operation">
      <span>{t(`u10-op-${status?.state ?? "running"}`)}</span>
      <progress max={status?.total ?? operation.total} value={status?.done ?? 0} />
      <span>{new Intl.NumberFormat(locale).format(status?.done ?? 0)} / {new Intl.NumberFormat(locale).format(status?.total ?? operation.total)}</span>
      {busy && <Button onPress={() => void op.cancel({ opId: operation.id }).catch(fail)}>{t("u10-cancel")}</Button>}
    </div>}
    <div className="u10-grid-host">
      {empty ? <EmptyState state={empty} /> : source && columns.length > 0 ?
        <DataGrid source={source} columns={columns.map(c => ({ ...c, sort: query.sort.find(s => s.column === c.key)?.dir }))}
          selection={controller.selection} onSelectionChange={controller.setSelection}
          onSortChange={sort => tabs.query({ sort })} onSortGesture={(column, extend) => tabs.query({ sort: cycleSort(query.sort, column, extend) })}
          headerTitle={column => { const index = query.sort.findIndex(s => s.column === column); return index < 0 ? column : `${column} ${query.sort[index].dir === "asc" ? "↑" : "↓"}${query.sort.length > 1 ? index + 1 : ""}`; }}
          onColumnsChange={next => void columnState.change(tabs.active, next).catch(fail)}
          onContextMenu={showContext} onActivate={() => undefined} onError={fail} controller={setGrid}
          confirmLargeCopy={async () => window.confirm(t("u10-large-copy"))} onExportSelection={() => openExport()}
          rowHeight={SIZES.rowHeight[appearance.density]} locale={locale} ariaLabel={t("u10-table")}
          theme={{ tokens: resolveTokens(resolveTheme(appearance.theme, systemPrefersDark()), appearance.accent),
            fontFamily: FONT_STACKS[currentPlatform()].ui, fontSize: SIZES.fontTable[appearance.density], zebra: appearance.zebra }} /> :
        <EmptyState state={{ code: controller.pending ? "NO_DATA_YET" : "NO_MATCH" }} />}
    </div>
    <div className="u10-counter" role="status">{t("u10-count", { shown: empty ? 0 : source?.total ?? 0, total: total ?? "–" })}
      {query.search && ` · ${t("u10-search-active")}`}{count > 0 && ` · ${t("u10-selected", { count })}`}</div>
    <span ref={anchor} className="u10-menu-anchor" style={{ left: context?.x ?? 0, top: context?.y ?? 0 }} />
    <Popover triggerRef={anchor} isOpen={context !== undefined} onOpenChange={open => { if (!open) setContext(undefined); }} className="os-popover">
      <MenuBody entries={menuEntries} label={t("u10-context")} />
    </Popover>
    <Dialog title={t("u10-remove")} isOpen={remove} onOpenChange={setRemove}
      footer={<DialogFooter okLabel={t("u10-remove")} cancelLabel={t("u10-cancel")} okVariant="danger"
        onOk={() => void (async () => { if (skipRemoval) await saveTableState({ tableConfirmRemove: false }); await actOnSelection("remove"); })().catch(fail)} onCancel={() => setRemove(false)} />}>
      <p>{t("u10-remove-confirm", { count })}</p>
      <label><input type="checkbox" checked={skipRemoval} onChange={event => setSkipRemoval(event.target.checked)} />{t("u10-dont-ask")}</label>
    </Dialog>
    <Dialog title={t("u10-browser")} isOpen={browserConfirm} onOpenChange={open => { setBrowserConfirm(open); if (!open) browserResolve.current?.(false); }}
      footer={<DialogFooter okLabel={t("u10-open")} cancelLabel={t("u10-cancel")}
        onOk={() => { browserResolve.current?.(true); setBrowserConfirm(false); }} onCancel={() => { browserResolve.current?.(false); setBrowserConfirm(false); }} />}>
      {t("u10-browser-confirm", { count: Math.min(10, count) })}
    </Dialog>
    <Dialog title={t("u10-export")} isOpen={exportOpen} onOpenChange={setExportOpen}
      footer={<DialogFooter okLabel={t("u10-export")} cancelLabel={t("u10-cancel")} okDisabled={!path.trim()}
        onOk={() => void exportSelection().catch(fail)} onCancel={() => setExportOpen(false)} />}>
      <label>{t("u10-export-path")}<input value={path} onChange={e => setPath(e.target.value)} /></label>
      {!exportSpec && <label><input type="checkbox" checked={allColumns} onChange={e => setAllColumns(e.target.checked)} />{t("u10-export-all")}</label>}
    </Dialog>
  </section>;
}
