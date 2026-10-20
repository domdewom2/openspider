import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useT, useFormat } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import type { ResponseTimeScope } from "../../../ipc/gen/ResponseTimeScope";
import { useSession } from "../../../state/session";
import { QueryFeedback } from "../frame/QueryFeedback";
import { PanelActions } from "../frame/PanelActions";
import { Bars } from "../site-structure/chart";
import { DEFAULT_COLOR, TABLE_TARGET } from "../site-structure/model";
import { navigateToTable } from "../site-structure/navigate";
import { bucketRows, bucketSearch } from "./model";

const SCOPES: ResponseTimeScope[] = ["all", "internalHtml"];

/** UI-069: distribution of response times in one-second buckets. */
export function ResponseTimes() {
  const t = useT();
  const f = useFormat();
  const [scope, setScope] = useState<ResponseTimeScope>("all");
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const version = useSession(s => s.sidebarVersion);
  const query = useQuery({
    queryKey: ["sidebar", "responseTimes", crawl, scope, counts, version],
    queryFn: ({ signal }) => sidebar.responseTimes({ scope }, signal),
    enabled: !!crawl,
    gcTime: 0,
    placeholderData: keepPreviousData,
  });
  const data = query.data;
  const rows = data ? bucketRows(data.buckets) : [];
  const go = (key: string) => {
    const row = rows.find(r => r.label === key);
    if (row) navigateToTable(TABLE_TARGET.tab, TABLE_TARGET.filter, bucketSearch(row.bucket));
  };
  return <div className="os-sidebar-response-times">
    <QueryFeedback error={query.error} loading={!!crawl && query.isPending} retry={() => void query.refetch()} />
    {!crawl && <p>{t("sidebar-no-crawl")}</p>}
    {data && <>
      <PanelActions which="responseTimes" rows={[[t("rt-times"), t("rt-number-of-urls"), t("sidebar-percent-total")], ...rows.map(r => [r.label, r.bucket.count, `${r.pct.toFixed(1)} %`]), [t("rt-no-response", { count: data.noResponse })]]} />
      <div className="os-sidebar-view"><label>{t("rt-scope")} <select aria-label={t("rt-scope")} value={scope} onChange={e => setScope(e.target.value as ResponseTimeScope)}>{SCOPES.map(s => <option key={s} value={s}>{t(`rt-scope-${s}`)}</option>)}</select></label></div>
      <table className="os-sidebar-table" aria-label={t("rt-table")}>
        <thead><tr><th>{t("rt-times")}</th><th>{t("rt-number-of-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead>
        <tbody>{rows.map(r => <tr key={r.label}>
          <th scope="row"><button type="button" onClick={() => go(r.label)}>{r.label}</button></th>
          <td className="os-sidebar-count">{f.integer(r.bucket.count)}</td><td className="os-sidebar-count">{f.decimal(r.pct, 1)} %</td>
        </tr>)}</tbody>
        <tfoot><tr><td colSpan={3}>{t("rt-no-response", { count: data.noResponse })}</td></tr></tfoot>
      </table>
      <Bars label={t("rt-chart")} rows={rows.map(r => ({ key: r.label, label: r.label, total: r.bucket.count, parts: [{ key: "all", label: t("rt-seconds", { range: r.label }), value: r.bucket.count, color: DEFAULT_COLOR }] }))} onSelect={go} />
      <p className="os-sidebar-note">{t("rt-note")}</p>
    </>}
  </div>;
}
