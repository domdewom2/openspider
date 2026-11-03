import { memo, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { useFormat, useT, type Formatters } from "../../../i18n";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import { diag } from "../../../ipc/diag";
import { Button, Menu, type MenuEntry } from "../../../ui/primitives";
import { ContextMenu } from "../../../ui/primitives/ContextMenu";
import { IssueTypeIcon } from "../../../ui/icons";
import { cssVarName, segmentLabelColor, THEME_TOKENS, type TokenName } from "../../../ui/tokens/tokens";
import { useComparison } from "../../compare/model";
import { useSession } from "../../../state/session";
import { useSegments } from "../../table-advanced/stores";
import { saveBlob } from "../../viz/exporters";
import { cssToHex } from "../../viz/model/render";
import { FilterStatus } from "../analysis/FilterStatus";
import { useChartLayout } from "../frame/layout";
import { QueryFeedback } from "../frame/QueryFeedback";
import { useSidebarNavigation } from "../frame/state";
import { chartEntry, entryKey, focusSelectedEntry, useOverviewSelection } from "../overview/selection";
import { navigateToTable } from "../site-structure/navigate";
import { dataRows, fileName, toCsv, toTsv } from "./data";
import { buildChart, describeTop, highlightKey, legendOrder, type ChartCategory, type ChartModel, type NavTarget } from "./model";
import { chartSvg, hatchAngle, liftOffset, ringSegments, RING, svgToPng, type SvgItem } from "./svg";
import "./overview-chart.css";

const TOOLTIP_DELAY = 300;
const NARROW = 260;

const tokenVar = (token: TokenName) => `var(${cssVarName(token)})`;
const percentText = (f: Formatters, pct: number | null) => pct == null ? "–" : `${f.decimal(pct, 1)} %`;
const countText = (f: Formatters, count: number | null) => count == null ? "–" : f.integer(count);

/** Hatched fill of a bar/swatch in the live chart (the SVG export uses a <pattern>). */
function fillStyle(c: Pick<ChartCategory, "color" | "hatch">): string {
  const color = tokenVar(c.color);
  if (c.hatch <= 0) return color;
  return `repeating-linear-gradient(${hatchAngle(c.hatch)}deg, ${color} 0 4px, var(--os-bg-surface) 4px 6px)`;
}

/** Applies a category's table target (UI-060); residual categories and „Weitere“ do not navigate. */
function applyTarget(nav: NavTarget | null): void {
  if (!nav) return;
  if (nav.search) navigateToTable(nav.tab, nav.filter, nav.search);
  else useSidebarNavigation.getState().navigate(nav.tab, nav.filter);
}

/** Token colours as hex values of the current theme (SVG export, UI-090 rules). */
function hexOf(token: TokenName): string {
  const root = document.documentElement;
  const theme = root.dataset.theme === "dark" ? "dark" : "light";
  return cssToHex(getComputedStyle(root).getPropertyValue(cssVarName(token)), THEME_TOKENS[theme][token].toLowerCase());
}

const warned = new Set<string>();
/** UI-102: a category sum larger than the base is an error case; one log warning per entry and values. */
function logOverflow(entry: string, sum: number, base: number): void {
  const key = `${entry}|${sum}|${base}`;
  if (warned.has(key)) return;
  warned.add(key);
  const message = `Overview chart: categories of ${entry} add up to ${sum}, more than the base ${base}`;
  console.warn(message);
  void diag.logFrontend({ level: "warn", message, context: { entry, sum: String(sum), base: String(base) } }).catch(() => undefined);
}

interface Tip { key: string; x: number; y: number }

/**
 * Esc inside the chart (UI-102: hide „Weitere“, back to the tree). The global shortcut dispatcher consumes Esc in the
 * capture phase of the document (search.clear); the chart answers it first on the window and keeps it for itself.
 */
function useEscape(ref: RefObject<HTMLElement | null>, handler: () => void): void {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    const listener = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape" || !(e.target instanceof Node) || !ref.current?.contains(e.target)) return;
      e.preventDefault();
      e.stopPropagation();
      latest.current();
    };
    window.addEventListener("keydown", listener, true);
    return () => window.removeEventListener("keydown", listener, true);
  }, [ref]);
}

export interface ChartAreaProps { data: OverviewDto; startAnalysis: () => void; height?: number }

/**
 * UI-102: chart area below the Overview tree. Shows only the counters of the loaded Overview answer; selecting an entry
 * calls no command. Redraws only when a value of the selected entry changes (memoized model and view).
 */
export function OverviewChartArea({ data, startAnalysis }: ChartAreaProps) {
  const t = useT();
  const comparison = useComparison(s => !!s.session);
  const selected = useOverviewSelection(s => s.entry);
  const entry = selected ? chartEntry(selected) : null;
  const key = entry ? entryKey(entry) : "";
  // The source objects keep their identity while their values do not change (structural sharing of the query).
  const source = entry?.kind === "summary" ? data.summary : entry ? data.tabs.find(x => x.tab === entry.tab) : undefined;
  const model = useMemo(() => entry ? buildChart(data, entry, t, logOverflow) : null, [key, source, t]);
  const highlight = model && selected ? highlightKey(model, selected) : null;
  if (comparison) return <section className="os-ovc-area" aria-label={t("ovc-region")}><p className="os-ovc-message">{t("ovc-compare")}</p></section>;
  return <ChartView model={model} highlight={highlight} startAnalysis={startAnalysis} />;
}

const ChartView = memo(function ChartView({ model, highlight, startAnalysis }: { model: ChartModel | null; highlight: string | null; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const view = useChartLayout(s => s.layout.view);
  const update = useChartLayout(s => s.update);
  const scope = useSidebarNavigation(s => s.scope);
  const segments = useSegments(s => s.segments);
  // UI-102: no transition while a crawl runs (and none with prefers-reduced-motion, see CSS).
  const running = useSession(s => s.crawl?.phase === "running");
  const area = useRef<HTMLElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const [narrow, setNarrow] = useState(false);
  const [error, setError] = useState<unknown>();
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    const measure = () => setNarrow(el.clientWidth > 0 && el.clientWidth < NARROW);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const segment = scope.segment?.kind === "segment" ? scope.segment.name : scope.segment?.kind === "none" ? t("ovc-segment-none") : null;
  const segmentColor = scope.segment?.kind === "segment" ? segments.find(s => s.name === segment)?.color : undefined;
  const chart = model?.kind === "chart" ? model : null;
  const header = model && model.kind !== "missing" ? model.header : null;

  const exportSvg = useCallback(() => {
    if (!chart) return null;
    const rect = body.current?.getBoundingClientRect();
    const items: SvgItem[] = chart.drawn.map(c => ({ label: c.label, count: countText(f, c.count), pct: percentText(f, c.pct), color: hexOf(c.color), hatch: c.hatch, value: c.count ?? 0, highlight: c.key === highlight }));
    const tail = chart.tail.map(c => ({ label: c.label, count: countText(f, c.count), pct: c.count == null ? "" : percentText(f, c.pct) }));
    const note = chart.chart === "overlap" ? t(chart.overflow ? "ovc-overflow" : "ovc-overlap-note") : undefined;
    return chartSvg({
      kind: chart.chart === "ring" ? "ring" : "bars",
      title: `${chart.header.label} – ${t("ovc-urls", { count: f.integer(chart.header.base) })}${segment ? ` – ${t("ovc-segment", { name: segment })}` : ""}`,
      items, tail, note,
      center: [f.integer(chart.header.base), "URLs"],
      colors: { surface: hexOf("bg.surface"), text: hexOf("text"), muted: hexOf("text.muted"), focus: hexOf("focus"), track: hexOf("bg.zebra") },
      font: getComputedStyle(document.documentElement).getPropertyValue("--os-font-ui").trim() || "sans-serif",
      width: Math.max(240, rect?.width ?? 320), height: rect?.height ?? 0,
    });
  }, [chart, f, highlight, segment, t]);

  const run = useCallback((job: () => Promise<void> | void) => {
    void (async () => { try { await job(); setError(undefined); } catch (e) { setError(e); } })();
  }, []);
  const name = (ext: string) => fileName(header?.name ?? "entry", segment, ext);
  const entries: MenuEntry[] = chart ? [
    { id: "png1", label: t("ovc-png-1"), onAction: () => run(async () => { const svg = exportSvg(); if (svg) saveBlob(name("png"), await svgToPng(svg, 1)); }) },
    { id: "png2", label: t("ovc-png-2"), onAction: () => run(async () => { const svg = exportSvg(); if (svg) saveBlob(name("png"), await svgToPng(svg, 2)); }) },
    { id: "svg", label: t("ovc-svg"), onAction: () => run(() => { const svg = exportSvg(); if (svg) saveBlob(name("svg"), new Blob([svg], { type: "image/svg+xml" })); }) },
    { id: "csv", label: t("ovc-csv"), separatorBefore: true, onAction: () => run(() => saveBlob(name("csv"), new Blob([toCsv(dataRows(chart))], { type: "text/csv;charset=utf-8" }))) },
    { id: "copy", label: t("ovc-copy"), onAction: () => run(() => navigator.clipboard.writeText(toTsv(dataRows(chart)))) },
  ] : [];
  // Test hook (E2E/vitest): the export documents without a save dialog.
  useEffect(() => {
    const w = window as unknown as { __osOverviewChart?: unknown };
    w.__osOverviewChart = chart ? { svg: exportSvg, csv: () => toCsv(dataRows(chart)), tsv: () => toTsv(dataRows(chart)), png: async (factor: 1 | 2) => { const svg = exportSvg(); return svg ? svgToPng(svg, factor) : null; } } : undefined;
  }, [chart, exportSvg]);

  let content: ReactNode;
  if (!model) content = <p className="os-ovc-message">{t("ovc-empty")}</p>;
  else if (model.kind === "missing") content = <p className="os-ovc-message">{t("ovc-empty")}</p>;
  else if (model.kind === "zero") content = <p className="os-ovc-message">{t("ovc-zero")}</p>;
  else if (model.kind === "noCounts") content = <NoCounts categories={model.categories} startAnalysis={startAnalysis} />;
  else content = view === "table"
    ? <ChartTable key={entryKey(model.header.entry)} model={model} highlight={highlight} />
    : <ChartBody key={entryKey(model.header.entry)} model={model} highlight={highlight} narrow={narrow} startAnalysis={startAnalysis} />;

  return <section ref={area} className="os-ovc-area" aria-label={t("ovc-region")} data-narrow={narrow || undefined} data-animate={!running || undefined} data-ovc-entry-shown={header ? entryKey(header.entry) : undefined}>
    {header && <div className="os-ovc-head">
      <div className="os-ovc-title">
        <span className="os-ovc-name">{header.label}</span>
        <span className="os-ovc-base">{t("ovc-urls", { count: f.integer(header.base) })}</span>
        {segment && <span className="os-ovc-chip" data-ovc-chip style={segmentColor ? { background: segmentColor, color: segmentLabelColor(segmentColor) } : undefined}>{t("ovc-segment", { name: segment })}</span>}
      </div>
      <div className="os-ovc-tools">
        <div className="os-ovc-switch" role="group" aria-label={t("ovc-view")}>
          <Button variant="tool" aria-pressed={view === "chart"} onPress={() => update({ view: "chart" })}>{t("ovc-view-chart")}</Button>
          <Button variant="tool" aria-pressed={view === "table"} onPress={() => update({ view: "table" })}>{t("ovc-view-table")}</Button>
        </div>
        <Menu label={t("ovc-export-menu")} entries={entries} trigger={<Button variant="tool" isDisabled={!chart}>{t("ovc-export")} ▾</Button>} />
      </div>
    </div>}
    <QueryFeedback error={error} />
    {chart ? <ContextMenu label={t("ovc-export-menu")} entries={entries} className="os-ovc-body-host"><div ref={body} className="os-ovc-body">{content}</div></ContextMenu>
      : <div ref={body} className="os-ovc-body">{content}</div>}
  </section>;
});

function NoCounts({ categories, startAnalysis }: { categories: ChartCategory[]; startAnalysis: () => void }) {
  const t = useT();
  const analysis = categories.some(c => c.state?.kind === "needsCrawlAnalysis");
  return <div className="os-ovc-message">
    <p>{t("ovc-no-counts")}</p>
    {analysis && <Button variant="tool" onPress={startAnalysis}>{t("ovc-start-analysis")}</Button>}
  </div>;
}

/** Ring or bars with legend; the legend (bars: the bar rows) is one focus group (UI-126). */
function ChartBody({ model, highlight, narrow, startAnalysis }: { model: Extract<ChartModel, { kind: "chart" }>; highlight: string | null; narrow: boolean; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const [expanded, setExpanded] = useState(false);
  const [zeroOpen, setZeroOpen] = useState(false);
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [tip, setTip] = useState<Tip | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const host = useRef<HTMLDivElement>(null);
  const descId = useId();
  const tipId = useId();
  const patternId = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const items = legendOrder(model, expanded);
  const zeroItems = model.chart === "overlap" && model.zero.length ? (zeroOpen ? model.zero : []) : [];
  const order = [...items, ...zeroItems];
  const zeroToggle = model.chart === "overlap" && model.zero.length > 0;
  const keys = [...order.map(c => c.key), ...(zeroToggle ? ["zero-toggle"] : [])];
  const active = focusKey && keys.includes(focusKey) ? focusKey : keys[0] ?? null;
  const byKey = useMemo(() => new Map([...model.drawn.flatMap(c => [c, ...(c.members ?? [])]), ...model.tail, ...model.zero].map(c => [c.key, c])), [model]);

  useEffect(() => () => clearTimeout(timer.current), []);
  const place = (key: string, el: Element | null, immediate: boolean) => {
    clearTimeout(timer.current);
    const show = () => {
      const box = host.current?.getBoundingClientRect();
      const r = el?.getBoundingClientRect();
      setTip({ key, x: r && box ? r.left - box.left + Math.min(r.width, 160) / 2 : 0, y: r && box ? r.bottom - box.top + 4 : 0 });
    };
    if (immediate) show(); else timer.current = setTimeout(show, TOOLTIP_DELAY);
  };
  const hide = () => { clearTimeout(timer.current); setTip(null); };

  const activate = (c: ChartCategory) => {
    if (c.role === "others") { setExpanded(e => !e); return; }
    applyTarget(c.nav);
  };
  const focusItem = (key: string) => {
    setFocusKey(key);
    host.current?.querySelector<HTMLElement>(`[data-ovc-key="${key.replace(/["\\]/g, "\\$&")}"]`)?.focus();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    const i = active ? keys.indexOf(active) : -1;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") { if (i < keys.length - 1) focusItem(keys[i + 1]!); }
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") { if (i > 0) focusItem(keys[i - 1]!); }
    else if (e.key === "Home") { if (keys[0]) focusItem(keys[0]); }
    else if (e.key === "End") { const last = keys.at(-1); if (last) focusItem(last); }
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  useEscape(host, () => {
    if (expanded) setExpanded(false);
    else { hide(); focusSelectedEntry(); }
  });
  const name = t("ovc-name", { entry: model.header.label, count: f.integer(model.header.base) });
  const describe = describeTop(model, c => `${c.label}: ${countText(f, c.count)} (${percentText(f, c.pct)})`);
  const tipCategory = tip ? byKey.get(tip.key) : undefined;

  const itemProps = (c: ChartCategory) => ({
    "data-ovc-key": c.key,
    tabIndex: c.key === active ? 0 : -1,
    "aria-describedby": tip?.key === c.key ? tipId : undefined,
    "aria-expanded": c.role === "others" ? expanded : undefined,
    onFocus: (e: { currentTarget: Element }) => { setFocusKey(c.key); place(c.key, e.currentTarget, true); },
    onBlur: hide,
    onPointerEnter: (e: { currentTarget: Element }) => place(c.key, e.currentTarget, false),
    onPointerLeave: hide,
    onClick: () => activate(c),
  });

  const legendRow = (c: ChartCategory, muted: boolean) => <li key={c.key}>
    <button type="button" className="os-ovc-item" data-highlight={c.key === highlight || undefined} data-focus={c.key === active || undefined} data-muted={muted || undefined} {...itemProps(c)}>
      <i className="os-ovc-swatch" style={{ background: fillStyle(c) }} aria-hidden />
      <span className="os-ovc-label" title={c.label}>{c.label}</span>
      <span className="os-ovc-count">{countText(f, c.count)}</span>
      <span className="os-ovc-pct">{c.count == null ? "" : percentText(f, c.pct)}</span>
    </button>
    {c.count == null && c.state && <span className="os-ovc-state"><FilterStatus state={c.state} startAnalysis={startAnalysis} /></span>}
  </li>;

  const max = Math.max(1, ...model.drawn.map(c => c.count ?? 0), ...model.drawn.flatMap(c => c.members?.map(m => m.count ?? 0) ?? []));
  const barRow = (c: ChartCategory, member: boolean) => <li key={c.key} data-member={member || undefined}>
    <button type="button" className="os-ovc-bar" data-highlight={c.key === highlight || undefined} data-focus={c.key === active || undefined} {...itemProps(c)}>
      <span className="os-ovc-bar-name">
        <i className="os-ovc-swatch" style={{ background: fillStyle(c) }} aria-hidden />
        {c.issue && <IssueTypeIcon type={c.issue.issueType} size={12} aria-hidden />}
        <span className="os-ovc-label" title={c.label}>{c.label}</span>
      </span>
      <span className="os-ovc-track" aria-hidden><span className="os-ovc-fill" style={{ width: `${((c.count ?? 0) / max) * 100}%`, background: fillStyle(c) }} /></span>
      <span className="os-ovc-count">{countText(f, c.count)}</span>
      <span className="os-ovc-pct">{percentText(f, c.pct)}</span>
    </button>
  </li>;

  const tooltip = tipCategory && <div className="os-ovc-tip" role="tooltip" id={tipId} style={{ left: tip!.x, top: tip!.y }}>
    <strong>{tipCategory.label}</strong>
    <span>{countText(f, tipCategory.count)}{tipCategory.count != null && ` · ${percentText(f, tipCategory.pct)}`}</span>
    <span>{t("ovc-tip-of", { base: f.integer(model.header.base) })}</span>
    {tipCategory.members && <><span>{t("ovc-tip-contains")}</span><ul>{tipCategory.members.map(m => <li key={m.key}>{m.label}: {countText(f, m.count)}</li>)}</ul></>}
    {tipCategory.role === "unassigned" && <span>{t("ovc-tip-unassigned")}</span>}
    {tipCategory.role === "hidden" && <span>{t("ovc-tip-hidden")}</span>}
    {(tipCategory.role === "unassigned" || tipCategory.role === "hidden") && <span>{t("ovc-tip-no-nav")}</span>}
  </div>;

  const desc = <p id={descId} className="os-visually-hidden">{describe}</p>;
  if (model.chart === "ring") {
    const segments = ringSegments(model.drawn.map(c => c.count ?? 0));
    return <div ref={host} className="os-ovc-ring-layout os-ovc-enter" data-narrow={narrow || undefined} onKeyDown={onKeyDown} data-ovc-chart="ring">
      <svg className="os-ovc-ring" viewBox={`0 0 ${RING.size} ${RING.size}`} role="img" aria-label={name} aria-describedby={descId}>
        {segments.map((s, i) => {
          const c = model.drawn[i]!;
          const lift = liftOffset(s.mid);
          return <path key={c.key} className="os-ovc-segment" data-ovc-segment={c.key} data-lift={c.key === highlight || undefined} d={s.d} fillRule="evenodd"
            style={{ fill: c.hatch ? `url(#${patternId}-${c.key.replace(/\W/g, "")})` : tokenVar(c.color), ["--ovc-dx" as string]: `${lift.dx}px`, ["--ovc-dy" as string]: `${lift.dy}px` }}
            onClick={() => activate(c)} onPointerEnter={e => place(c.key, e.currentTarget, false)} onPointerLeave={hide} />;
        })}
        <defs>{model.drawn.filter(c => c.hatch > 0).map(c => <pattern key={c.key} id={`${patternId}-${c.key.replace(/\W/g, "")}`} patternUnits="userSpaceOnUse" width={6} height={6} patternTransform={`rotate(${hatchAngle(c.hatch)})`}>
          <rect width={6} height={6} style={{ fill: tokenVar(c.color) }} /><rect width={2} height={6} style={{ fill: "var(--os-bg-surface)" }} />
        </pattern>)}</defs>
        {segments.map((s, i) => {
          const c = model.drawn[i]!;
          const marked = c.key === highlight || c.key === active && focusKey !== null;
          const lift = liftOffset(s.mid);
          return marked ? <path key={`m-${c.key}`} className="os-ovc-mark" data-kind={c.key === highlight ? "highlight" : "focus"} data-lift={c.key === highlight || undefined} d={s.d} fillRule="evenodd"
            style={{ ["--ovc-dx" as string]: `${lift.dx}px`, ["--ovc-dy" as string]: `${lift.dy}px` }} /> : null;
        })}
        <text x={RING.size / 2} y={RING.size / 2 + 6} textAnchor="middle" className="os-ovc-center">{f.integer(model.header.base)}</text>
        <text x={RING.size / 2} y={RING.size / 2 + 24} textAnchor="middle" className="os-ovc-center-unit">URLs</text>
      </svg>
      {desc}
      <ul className="os-ovc-legend" aria-label={t("ovc-legend", { entry: model.header.label })}>
        {items.map(c => legendRow(c, c.count === 0))}
      </ul>
      {tooltip}
    </div>;
  }
  return <div ref={host} className="os-ovc-bars-layout os-ovc-enter" onKeyDown={onKeyDown} data-ovc-chart={model.chart}>
    {desc}
    <div role="group" aria-label={name} aria-describedby={descId}>
      <ul className="os-ovc-bars">
        {model.drawn.flatMap(c => [barRow(c, false), ...(c.members && expanded ? c.members.map(m => barRow(m, true)) : [])])}
      </ul>
    </div>
    {model.tail.length > 0 && <ul className="os-ovc-legend" aria-label={t("ovc-legend", { entry: model.header.label })}>{model.tail.map(c => legendRow(c, c.count === 0))}</ul>}
    {zeroToggle && <div className="os-ovc-zero">
      <button type="button" className="os-ovc-item" data-ovc-key="zero-toggle" tabIndex={active === "zero-toggle" ? 0 : -1} aria-expanded={zeroOpen}
        onFocus={() => setFocusKey("zero-toggle")} onClick={() => setZeroOpen(o => !o)}>
        <span aria-hidden>{zeroOpen ? "▾" : "▸"}</span> {t("ovc-zero-filters", { count: model.zero.length })}
      </button>
      {zeroOpen && <ul className="os-ovc-legend">{model.zero.map(c => legendRow(c, true))}</ul>}
    </div>}
    {model.chart === "overlap" && <p className="os-ovc-note">{t(model.overflow ? "ovc-overflow" : "ovc-overlap-note")}</p>}
    {tooltip}
  </div>;
}

/** Table view: every category individually (no bundling), the same click targets as the chart. */
function ChartTable({ model, highlight }: { model: Extract<ChartModel, { kind: "chart" }>; highlight: string | null }) {
  const t = useT();
  const f = useFormat();
  const host = useRef<HTMLDivElement>(null);
  useEscape(host, () => { focusSelectedEntry(); });
  return <div ref={host} className="os-ovc-table-host os-ovc-enter">
    <table className="os-sidebar-table os-ovc-table" aria-label={t("ovc-name", { entry: model.header.label, count: f.integer(model.header.base) })}>
      <thead><tr><th scope="col">{t("ovc-col-category")}</th><th scope="col" className="os-ovc-num">{t("sidebar-urls")}</th><th scope="col" className="os-ovc-num">{t("sidebar-percent-total")}</th></tr></thead>
      <tbody>{model.table.map(c => <tr key={c.key} data-highlight={c.key === highlight || undefined} data-ovc-row={c.key}>
        <th scope="row"><span className="os-ovc-cell"><i className="os-ovc-swatch" style={{ background: fillStyle(c) }} aria-hidden />{c.nav ? <button type="button" onClick={() => applyTarget(c.nav)}>{c.label}</button> : <span>{c.label}</span>}{c.issue && <IssueTypeIcon type={c.issue.issueType} size={12} aria-hidden />}</span></th>
        <td className="os-ovc-num">{countText(f, c.count)}</td>
        <td className="os-ovc-num">{c.count == null ? "–" : percentText(f, c.pct)}</td>
      </tr>)}</tbody>
    </table>
  </div>;
}
