import { Fragment, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useT, useFormat } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import { useSession } from "../../../state/session";
import { ApiSourceGear } from "../../integrations";
import { QueryFeedback } from "../frame/QueryFeedback";
import { PanelActions } from "../frame/PanelActions";
import { apiStatus, panelRows, progressPercent } from "./model";
import { ApiErrors } from "./ApiErrors";

/** Changes whenever an API operation changes state, so the table follows fetches live. */
function useApiOpsVersion(): string {
  return useSession(s => Object.values(s.ops).filter(o => o.labelKey.startsWith("op-api-")).map(o => `${o.opId}:${o.state}:${o.done}`).join("|"));
}

/** UI-070: state of every connected data source; never shows credentials or tokens. */
export function ApiPanel() {
  const t = useT();
  const f = useFormat();
  const [open, setOpen] = useState<string | null>(null);
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const version = useSession(s => s.sidebarVersion);
  const ops = useApiOpsVersion();
  const query = useQuery({ queryKey: ["sidebar", "apiStatus", crawl, counts, version, ops], queryFn: ({ signal }) => sidebar.apiStatus(signal), gcTime: 0, placeholderData: keepPreviousData });
  const rows = panelRows(query.data ?? []);
  const statusText = (row: (typeof rows)[number]) => {
    const status = apiStatus(row);
    const base = t(`sapi-status-${status}`);
    return status === "quotaExhausted" && row.resetAt ? `${base} – ${t("sapi-resets", { time: f.dateTime(row.resetAt) })}` : base;
  };
  const name = (kind: string) => t(`api-${kind}`);
  return <div className="os-sidebar-api">
    <QueryFeedback error={query.error} loading={query.isPending} retry={() => void query.refetch()} />
    {query.data && <>
      <PanelActions which="api" rows={[[t("sapi-api"), t("sapi-status"), t("sapi-requested"), t("sapi-received"), t("sapi-errors"), t("sapi-progress")], ...rows.map(r => [name(r.kind), statusText(r), r.requested, r.received, r.errors, `${progressPercent(r)} %`])]} />
      <table className="os-sidebar-table" aria-label={t("sapi-table")}>
        <thead><tr><th>{t("sapi-api")}</th><th>{t("sapi-status")}</th><th>{t("sapi-requested")}</th><th>{t("sapi-received")}</th><th>{t("sapi-errors")}</th><th>{t("sapi-progress")}</th></tr></thead>
        <tbody>{rows.map(r => <Fragment key={r.kind}>
          <tr data-api-kind={r.kind} data-api-status={apiStatus(r)}>
            <th scope="row"><span className="os-sidebar-api-name">{name(r.kind)}<ApiSourceGear kind={r.kind} /></span></th>
            <td>{statusText(r)}</td>
            <td className="os-sidebar-count">{f.integer(r.requested)}</td>
            <td className="os-sidebar-count">{f.integer(r.received)}</td>
            <td className="os-sidebar-count">{r.errors > 0
              ? <button type="button" aria-expanded={open === r.kind} aria-label={t("sapi-show-errors", { name: name(r.kind), count: r.errors })} onClick={() => setOpen(o => o === r.kind ? null : r.kind)}>{f.integer(r.errors)}</button>
              : f.integer(r.errors)}</td>
            <td className="os-sidebar-count">{progressPercent(r)} %</td>
          </tr>
          {open === r.kind && r.errors > 0 && <tr><td colSpan={6}><ApiErrors kind={r.kind} count={r.errors} /></td></tr>}
        </Fragment>)}</tbody>
      </table>
    </>}
  </div>;
}
