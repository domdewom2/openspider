import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useT, useFormat } from "../../../i18n";
import { api } from "../../../ipc/api";
import type { ApiKind } from "../../../ipc/gen/ApiKind";
import { QueryFeedback } from "../frame/QueryFeedback";
import { redact } from "./model";

const PAGE = 50;

/** List of final errors of one source: URL, message, time (UI-070). */
export function ApiErrors({ kind, count }: { kind: ApiKind; count: number }) {
  const t = useT();
  const f = useFormat();
  const [limit, setLimit] = useState(PAGE);
  const query = useQuery({ queryKey: ["sidebar", "apiErrors", kind, limit, count], queryFn: ({ signal }) => api.errors({ kind, offset: 0, limit }, signal), gcTime: 0, placeholderData: keepPreviousData });
  const rows = query.data ?? [];
  return <section className="os-sidebar-api-errors" aria-label={t("sapi-errors-of", { name: t(`api-${kind}`) })}>
    <QueryFeedback error={query.error} loading={query.isPending} retry={() => void query.refetch()} />
    {query.data && <table className="os-sidebar-table">
      <thead><tr><th>{t("sapi-url")}</th><th>{t("sapi-message")}</th><th>{t("sapi-time")}</th></tr></thead>
      <tbody>{rows.map((e, i) => <tr key={`${e.at}-${e.url ?? ""}-${i}`}><td>{e.url ?? "–"}</td><td>{redact(e.message)}</td><td>{f.dateTime(e.at)}</td></tr>)}</tbody>
    </table>}
    {query.data && rows.length >= limit && count > rows.length && <button type="button" onClick={() => setLimit(l => l + PAGE)}>{t("sapi-more-errors")}</button>}
  </section>;
}
