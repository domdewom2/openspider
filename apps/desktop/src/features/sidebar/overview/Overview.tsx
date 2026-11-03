import { memo, useCallback, useEffect, useRef, type KeyboardEvent } from "react";
import { filterLabel, tabLabel } from "../../../i18n/canon";
import { ComparisonNumbers } from "../../compare/ComparisonNumbers";
import { useComparison } from "../../compare/model";
import { useT, useFormat, type TFn } from "../../../i18n";
import type { Formatters } from "../../../i18n/format";
import type { FilterInfo } from "../../../ipc/gen/FilterInfo";
import type { FilterStateDto } from "../../../ipc/gen/FilterStateDto";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { OverviewTab } from "../../../ipc/gen/OverviewTab";
import { useSession } from "../../../state/session";
import { IssueBadge } from "../issues/IssueBadge";
import { FilterStatus, hasCount } from "../analysis/FilterStatus";
import { useSidebarNavigation } from "../frame/state";
import { ChartPane } from "../overview-chart/ChartPane";
import { OverviewChartArea } from "../overview-chart/OverviewChart";
import { entryKey, SUMMARY_KEYS, useOverviewSelection, type EntryRef } from "./selection";

export { SUMMARY_KEYS };

export function overviewRows(data: OverviewDto, t: TFn): (string | number)[][] {
  const encountered = data.summary.find(r => r.key === SUMMARY_KEYS[0])?.count ?? 0;
  return [[t("sidebar-name"), t("sidebar-urls"), t("sidebar-percent-total")],
    ...SUMMARY_KEYS.flatMap(key => data.summary.filter(r => r.key === key).map(r => [r.key, r.count, `${(encountered ? r.count / encountered * 100 : 0).toFixed(1)} %`])),
    ...data.tabs.flatMap(tab => tab.filters.map(f => [`${tab.tab}: ${f.id}`, hasCount(f.state) ? f.count ?? "–" : "–", hasCount(f.state) && f.pct != null ? `${f.pct.toFixed(1)} %` : "–"]))];
}

const NO_STALE: readonly string[] = [];
const STALE: FilterStateDto = { kind: "stale" };

/** Same function identity across renders, always calling the latest callback (memoized rows stay skipped). */
function useLatest<A extends unknown[]>(callback: (...args: A) => void): (...args: A) => void {
  const latest = useRef(callback);
  latest.current = callback;
  return useCallback((...args: A) => latest.current(...args), []);
}

/** UI-102: is this entry selected? Rows subscribe to their own flag only, so a selection change renders two rows. */
function useSelected(entry: EntryRef): boolean {
  const key = entryKey(entry);
  return useOverviewSelection(s => !!s.entry && entryKey(s.entry) === key);
}
const select = (entry: EntryRef) => useOverviewSelection.getState().select(entry);

/**
 * One filter row. UI-145/UI-018 (M2-FIX-LIVE): while a crawl runs, the shared count answers every few seconds, and
 * most answers change only `ageMs` or a few dozen of the ~400 filters. TanStack Query keeps unchanged filter objects
 * (structural sharing), so a row renders again only when its own filter, stale mark or selection changes.
 */
const FilterRow = memo(function FilterRow({ tab, filter, staleStep, startAnalysis }: { tab: string; filter: FilterInfo; staleStep: boolean; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const navigate = useSidebarNavigation(s => s.navigate);
  const entry: EntryRef = { kind: "filter", tab, filter: filter.id };
  const selected = useSelected(entry);
  const counted = hasCount(filter.state);
  return <tr className={(filter.state.kind === "stale" || staleStep) ? "os-sidebar-stale" : undefined}>
    {/* UI-102: a click selects the row and navigates as before (UI-060). */}
    <th scope="row"><button type="button" data-ovc-entry={entryKey(entry)} aria-current={selected || undefined} onClick={() => { select(entry); navigate(tab, filter.id); }}>{filterLabel(t, filter)}</button>{filter.issue && <IssueBadge badge={filter.issue} />}<FilterStatus state={filter.state} startAnalysis={startAnalysis} />{filter.state.kind !== "stale" && staleStep && <FilterStatus state={STALE} />}</th>
    <td>{counted && filter.count != null ? f.integer(filter.count) : "–"}</td><td>{counted ? percent(f, filter.pct) : "–"}</td>
  </tr>;
});

/** One tab with its filters; skipped as a whole while neither its filters nor its collapsed/stale state change. */
const TabSection = memo(function TabSection({ tab, collapsed, staleSteps, toggle, startAnalysis }: { tab: OverviewTab; collapsed: boolean; staleSteps: readonly string[]; toggle: (id: string) => void; startAnalysis: () => void }) {
  const t = useT();
  const entry: EntryRef = { kind: "tab", tab: tab.tab };
  const selected = useSelected(entry);
  const label = tabLabel(t, tab.tab);
  // UI-102: the name selects the block; the arrow (and arrow right/left) expands and collapses it (UI-061).
  return <section>
    <div className="os-ovc-group-row">
      <button type="button" className="os-sidebar-toggle" aria-expanded={!collapsed} aria-label={`${collapsed ? "▸" : "▾"} ${label}`} data-ovc-toggle={tab.tab} onClick={() => toggle(tab.tab)}>{collapsed ? "▸" : "▾"}</button>
      <button type="button" className="os-sidebar-group" data-ovc-entry={entryKey(entry)} aria-current={selected || undefined} onClick={() => select(entry)}>{label}</button>
    </div>
    {!collapsed && <table className="os-sidebar-table" aria-label={label}><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>{tab.filters.map(filter =>
      <FilterRow key={filter.id} tab={tab.tab} filter={filter} staleStep={staleSteps.includes(filter.analysisStep ?? "")} startAnalysis={startAnalysis} />)}</tbody></table>}
  </section>;
});

function percent(f: Formatters, n?: number | null): string {
  return n == null ? "–" : `${f.decimal(n, 1)} %`;
}

const SUMMARY: EntryRef = { kind: "summary" };

function SummaryCaption() {
  const t = useT();
  const selected = useSelected(SUMMARY);
  return <caption><button type="button" data-ovc-entry="summary" aria-current={selected || undefined} onClick={() => select(SUMMARY)}>{t("sidebar-summary")}</button></caption>;
}

/** Tree keyboard (UI-102): arrow up/down moves the selection, arrow right/left expands/collapses a tab block. */
function onTreeKey(e: KeyboardEvent<HTMLDivElement>, toggle: (id: string) => void, collapsed: readonly string[]): void {
  const target = e.target as HTMLElement;
  const host = e.currentTarget;
  const items = [...host.querySelectorAll<HTMLElement>("[data-ovc-entry]")];
  const own = target.closest<HTMLElement>("[data-ovc-entry],[data-ovc-toggle]");
  if (!own || !host.contains(own)) return;
  const tab = own.dataset.ovcToggle ?? (own.dataset.ovcEntry?.startsWith("tab:") ? own.dataset.ovcEntry.slice(4) : undefined);
  const index = items.indexOf(own.dataset.ovcToggle !== undefined ? items.find(i => i.dataset.ovcEntry === `tab:${own.dataset.ovcToggle}`)! : own);
  const move = (to: HTMLElement | undefined) => {
    if (!to) return;
    to.focus();
    const key = to.dataset.ovcEntry!;
    select(key === "summary" ? SUMMARY : key.startsWith("tab:") ? { kind: "tab", tab: key.slice(4) } : { kind: "filter", tab: key.slice(7, key.indexOf("\u001f")), filter: key.slice(key.indexOf("\u001f") + 1) });
  };
  if (e.key === "ArrowDown") move(items[index + 1]);
  else if (e.key === "ArrowUp") move(items[Math.max(0, index - 1)]);
  else if (e.key === "ArrowRight" && tab !== undefined) { if (collapsed.includes(tab)) toggle(tab); }
  else if (e.key === "ArrowLeft" && tab !== undefined) { if (!collapsed.includes(tab)) toggle(tab); }
  else if (e.key === "ArrowLeft" && own.dataset.ovcEntry?.startsWith("filter:")) {
    const key = own.dataset.ovcEntry;
    move(items.find(i => i.dataset.ovcEntry === `tab:${key.slice(7, key.indexOf("\u001f"))}`));
  } else return;
  e.preventDefault();
}

function OverviewContent({ data, collapsed, toggle, startAnalysis, staleSteps = NO_STALE }: { data: OverviewDto; collapsed: readonly string[]; staleSteps?: readonly string[]; toggle: (id: string) => void; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const onToggle = useLatest(toggle);
  const onStartAnalysis = useLatest(startAnalysis);
  const activeComparison = useComparison(s => !!s.session);
  const crawl = useSession(s => s.crawl?.crawlId ?? null);
  // UI-102: another crawl (or a cleared one) drops the selection.
  useEffect(() => useOverviewSelection.getState().forCrawl(crawl), [crawl]);
  const encountered = data.summary.find(r => r.key === SUMMARY_KEYS[0])?.count ?? 0;
  if (activeComparison) return <ComparisonNumbers />;
  const tree = <div className="os-sidebar-overview" onKeyDown={e => onTreeKey(e, onToggle, collapsed)}>
    <table className="os-sidebar-table"><SummaryCaption /><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>
      {SUMMARY_KEYS.map((key, i) => { const row = data.summary.find(r => r.key === key); return row ? <tr key={key}><th scope="row">{t(`sidebar-summary-${i}`)}</th><td>{f.integer(row.count)}</td><td>{percent(f, encountered ? row.count / encountered * 100 : 0)}</td></tr> : null; })}
    </tbody></table>
    {data.tabs.map(tab => <TabSection key={tab.tab} tab={tab} collapsed={collapsed.includes(tab.tab)} staleSteps={staleSteps} toggle={onToggle} startAnalysis={onStartAnalysis} />)}
  </div>;
  return <ChartPane tree={tree} chart={<OverviewChartArea data={data} startAnalysis={onStartAnalysis} />} />;
}

// Unrelated status ticks must not reconcile every filter row.
export const Overview = memo(OverviewContent);
