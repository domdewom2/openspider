import { useQuery } from "@tanstack/react-query";
import { useFormat, useT } from "../../i18n";
import { filterLabel, tabLabel } from "../../i18n/canon";
import { sidebar } from "../../ipc/sidebar";
import { useSession } from "../../state/session";
import { useTabs } from "../../state/tabs";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { useOverview } from "../sidebar/frame/useSidebarQuery";
import { QueryFeedback } from "../sidebar/frame/QueryFeedback";
import { useSidebarNavigation } from "../sidebar/frame/state";
import { sortIssues } from "../sidebar/issues/Issues";
import { IssueBadge } from "../sidebar/issues/IssueBadge";
import { hasCount } from "../sidebar/analysis/FilterStatus";
import "./dashboard.css";

/** Dashboard uses the same overview snapshot, issue command and navigation as the sidebar. */
export function Dashboard() {
  const t = useT(); const f = useFormat();
  const overview = useOverview();
  const crawl = useSession(s => s.crawl?.crawlId);
  const version = useSession(s => s.issuesVersion);
  const counts = useSession(s => s.countsVersion);
  const scope = useSidebarNavigation(s => s.scope);
  const issues = useQuery({ queryKey: ["sidebar", "issues", crawl, { ...scope, type: null, priority: null, search: null }, version, counts], queryFn: ({ signal }) => sidebar.issues({ ...scope, type: null, priority: null, search: null }, signal), enabled: !!crawl, gcTime: 0 });
  const navigate = (tab: string, filter: string) => {
    useTabs.getState().select(tab, filter);
    useSidebarNavigation.getState().navigate(tab, filter, true);
  };
  if (!crawl) return <section className="os-dashboard" aria-label={t("u34-nav-overview")}><EmptyState state={{ code: "NO_DATA_YET" }} /></section>;
  const data = overview.data;
  const distribution = data?.tabs.find(tab => tab.tab === "Response Codes")?.filters.filter(row => row.id !== "All") ?? [];
  const top = sortIssues(issues.data ?? []).slice(0, 5);
  return <section className="os-dashboard" aria-label={t("u34-nav-overview")} data-dashboard="">
    <QueryFeedback error={overview.error} loading={!data && overview.isPending} retry={() => void overview.refetch()} />
    {data && <>
      <div className="os-dashboard-metrics">{[0, 1, 7, 8].map(i => {
        const key = ["Total URLs Encountered", "Total URLs Crawled", "", "", "", "", "", "Total Internal Indexable URLs", "Total Internal Non-Indexable URLs"][i];
        const row = data.summary.find(row => row.key === key);
        return <article className="os-dashboard-card" key={i}><span>{t(`sidebar-summary-${i}`)}</span><strong>{row ? f.integer(row.count) : "–"}</strong></article>;
      })}</div>
      <div className="os-dashboard-sections">
        <section className="os-dashboard-card"><h2>{t("u35-distribution")}</h2>
          {distribution.map(row => <button className="os-dashboard-row" key={row.id} onClick={() => navigate("Response Codes", row.id)}><span>{filterLabel(t, row.id)}</span><strong>{hasCount(row.state) ? f.integer(row.count ?? 0) : "–"}</strong><progress max={100} value={hasCount(row.state) ? row.pct ?? 0 : 0} /></button>)}
          {!distribution.length && <EmptyState state={{ code: "NO_DATA_YET" }} />}
        </section>
        <section className="os-dashboard-card"><h2>{t("u35-top-issues")}</h2>
          <QueryFeedback error={issues.error} loading={issues.isPending} retry={() => void issues.refetch()} />
          {top.map(row => <button className="os-dashboard-row" key={row.id} onClick={() => navigate(row.tab, row.filter)}><span>{tabLabel(t, row.tab)}: {filterLabel(t, row.filter)}</span><strong>{f.integer(row.count)}</strong><IssueBadge badge={{ issueType: row.type, priority: row.priority }} /></button>)}
          {issues.data && !top.length && <p>{t("sidebar-no-issues")}</p>}
        </section>
      </div>
    </>}
  </section>;
}
