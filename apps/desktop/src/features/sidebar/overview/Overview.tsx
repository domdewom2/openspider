import { ComparisonNumbers } from "../../compare/ComparisonNumbers";
import { useComparison } from "../../compare/model";
import { useT, useFormat, type TFn } from "../../../i18n";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
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

export function Overview({ data, collapsed, toggle, startAnalysis, staleSteps = [] }: { data: OverviewDto; collapsed: readonly string[]; staleSteps?: readonly string[]; toggle: (id: string) => void; startAnalysis: () => void }) {
  const t = useT();
  const f = useFormat();
  const navigate = useSidebarNavigation(s => s.navigate);
  const pct = (n?: number | null) => n == null ? "–" : `${f.decimal(n, 1)} %`;
  const activeComparison = useComparison(s => !!s.session);
  const encountered = data.summary.find(r => r.key === SUMMARY_KEYS[0])?.count ?? 0;
  if (activeComparison) return <ComparisonNumbers />;
  return <div className="os-sidebar-overview">
    <table className="os-sidebar-table"><caption>{t("sidebar-summary")}</caption><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>
      {SUMMARY_KEYS.map((key, i) => { const row = data.summary.find(r => r.key === key); return row ? <tr key={key}><th scope="row">{t(`sidebar-summary-${i}`)}</th><td>{f.integer(row.count)}</td><td>{pct(encountered ? row.count / encountered * 100 : 0)}</td></tr> : null; })}
    </tbody></table>
    {data.tabs.map(tab => <section key={tab.tab}>
      <button type="button" className="os-sidebar-group" aria-expanded={!collapsed.includes(tab.tab)} onClick={() => toggle(tab.tab)}>{collapsed.includes(tab.tab) ? "▸" : "▾"} {tab.tab}</button>
      {!collapsed.includes(tab.tab) && <table className="os-sidebar-table" aria-label={tab.tab}><thead><tr><th>{t("sidebar-name")}</th><th>{t("sidebar-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead><tbody>{tab.filters.map(filter => <tr key={filter.id} className={(filter.state.kind === "stale" || staleSteps.includes(filter.analysisStep ?? "")) ? "os-sidebar-stale" : undefined}>
        <th scope="row"><button type="button" onClick={() => navigate(tab.tab, filter.id)}>{filter.id}</button>{filter.issue && <IssueBadge badge={filter.issue} />}<FilterStatus state={filter.state} startAnalysis={startAnalysis} />{filter.state.kind !== "stale" && staleSteps.includes(filter.analysisStep ?? "") && <FilterStatus state={{ kind: "stale" }} />}</th>
        <td>{hasCount(filter.state) && filter.count != null ? f.integer(filter.count) : "–"}</td><td>{hasCount(filter.state) ? pct(filter.pct) : "–"}</td>
      </tr>)}</tbody></table>}
    </section>)}
  </div>;
}
