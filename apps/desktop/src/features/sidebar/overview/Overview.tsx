import { memo, useCallback, useRef } from "react";
import { filterLabel, tabLabel } from "../../../i18n/canon";
import { ComparisonNumbers } from "../../compare/ComparisonNumbers";
import { useComparison } from "../../compare/model";
import { useT, useFormat, type TFn } from "../../../i18n";
import type { Formatters } from "../../../i18n/format";
import type { FilterInfo } from "../../../ipc/gen/FilterInfo";
import type { FilterStateDto } from "../../../ipc/gen/FilterStateDto";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { OverviewTab } from "../../../ipc/gen/OverviewTab";
import { IssueBadge } from "../issues/IssueBadge";
import { FilterStatus, hasCount } from "../analysis/FilterStatus";
import { useSidebarNavigation } from "../frame/state";

export const SUMMARY_KEYS = ["Total URLs Encountered", "Total URLs Crawled", "Total Internal Blocked by robots.txt", "Total External Blocked by robots.txt", "Total URLs Displayed", "Total Internal URLs", "Total External URLs", "Total Internal Indexable URLs", "Total Internal Non-Indexable URLs"];

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

/**
 * One filter row. UI-145/UI-018 (M2-FIX-LIVE): while a crawl runs, the shared count answers every few seconds, and
 * most answers change only `ageMs` or a few dozen of the ~400 filters. TanStack Query keeps unchanged filter objects
 * (structural sharing), so a row renders again only when its own filter or stale mark changes.
 */
const FilterRow = memo(function FilterRow({ tab, filter, staleStep, startAnalysis }: { tab: string; filter: FilterInfo; staleStep: boolean; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const navigate = useSidebarNavigation(s => s.navigate);
  const counted = hasCount(filter.state);
  return <tr className={(filter.state.kind === "stale" || staleStep) ? "os-sidebar-stale" : undefined}>
    <th scope="row"><button type="button" onClick={() => navigate(tab, filter.id)}>{filterLabel(t, filter)}</button>{filter.issue && <IssueBadge badge={filter.issue} />}<FilterStatus state={filter.state} startAnalysis={startAnalysis} />{filter.state.kind !== "stale" && staleStep && <FilterStatus state={STALE} />}</th>
    <td>{counted && filter.count != null ? f.integer(filter.count) : "–"}</td><td>{counted ? percent(f, filter.pct) : "–"}</td>
  </tr>;
});

/** One tab with its filters; skipped as a whole while neither its filters nor its collapsed/stale state change. */
const TabSection = memo(function TabSection({ tab, collapsed, staleSteps, toggle, startAnalysis }: { tab: OverviewTab; collapsed: boolean; staleSteps: readonly string[]; toggle: (id: string) => void; startAnalysis: () => void }) {
  const t = useT();
  return <section>
    <button type="button" className="os-sidebar-group" aria-expanded={!collapsed} onClick={() => toggle(tab.tab)}>{collapsed ? "▸" : "▾"} {tabLabel(t, tab.tab)}</button>
    {!collapsed && <table className="os-sidebar-table" aria-label={tabLabel(t, tab.tab)}><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>{tab.filters.map(filter =>
      <FilterRow key={filter.id} tab={tab.tab} filter={filter} staleStep={staleSteps.includes(filter.analysisStep ?? "")} startAnalysis={startAnalysis} />)}</tbody></table>}
  </section>;
});

function percent(f: Formatters, n?: number | null): string {
  return n == null ? "–" : `${f.decimal(n, 1)} %`;
}

function OverviewContent({ data, collapsed, toggle, startAnalysis, staleSteps = NO_STALE }: { data: OverviewDto; collapsed: readonly string[]; staleSteps?: readonly string[]; toggle: (id: string) => void; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const onToggle = useLatest(toggle);
  const onStartAnalysis = useLatest(startAnalysis);
  const activeComparison = useComparison(s => !!s.session);
  const encountered = data.summary.find(r => r.key === SUMMARY_KEYS[0])?.count ?? 0;
  if (activeComparison) return <ComparisonNumbers />;
  return <div className="os-sidebar-overview">
    <table className="os-sidebar-table"><caption>{t("sidebar-summary")}</caption><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>
      {SUMMARY_KEYS.map((key, i) => { const row = data.summary.find(r => r.key === key); return row ? <tr key={key}><th scope="row">{t(`sidebar-summary-${i}`)}</th><td>{f.integer(row.count)}</td><td>{percent(f, encountered ? row.count / encountered * 100 : 0)}</td></tr> : null; })}
    </tbody></table>
    {data.tabs.map(tab => <TabSection key={tab.tab} tab={tab} collapsed={collapsed.includes(tab.tab)} staleSteps={staleSteps} toggle={onToggle} startAnalysis={onStartAnalysis} />)}
  </div>;
}

// Unrelated status ticks must not reconcile every filter row.
export const Overview = memo(OverviewContent);
