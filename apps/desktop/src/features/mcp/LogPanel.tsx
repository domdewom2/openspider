import { useEffect, useState } from "react";
import { useFormat, useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import type { CallLogRow } from "../../ipc/gen/CallLogRow";
import { Banner, Button } from "../../ui/primitives";
import { errorKey, redactText } from "./model";

const PAGE_SIZE = 25;
export function LogPanel() {
  const t = useT();
  const format = useFormat();
  const [offset, setOffset] = useState(0);
  const [reload, setReload] = useState(0);
  const [rows, setRows] = useState<CallLogRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setRows(null); setError(null);
    void mcp.callLog({ offset, limit: PAGE_SIZE }).then(result => { if (live) setRows(result); }).catch(e => { if (live) setError(errorKey(e)); });
    return () => { live = false; };
  }, [offset, reload]);
  return <section className="os-mcp-stack" aria-label={t("mcp-log")}>
    {error && <Banner kind="error">{t(error)}</Banner>}
    {!rows && !error && <p role="status">{t("mcp-loading")}</p>}
    {rows && (rows.length === 0 ? <p>{t("mcp-no-calls")}</p> : <div className="os-mcp-scroll"><table className="os-mcp-table">
      <caption>{t("mcp-log")}</caption><thead><tr>{["time", "client", "tool", "parameters", "duration", "result-size", "result"].map(key => <th key={key}>{t(`mcp-${key}`)}</th>)}</tr></thead>
      <tbody>{rows.map((row, i) => <tr key={`${row.at}-${i}`}>
        <td><time dateTime={row.at}>{row.at}</time></td><td>{redactText(row.client ?? "—")}</td><td>{redactText(row.tool)}</td>
        <td><code>{redactText(row.argsSummary)}</code></td><td>{format.integer(row.durationMs)} ms</td>
        <td>{row.resultBytes == null ? "—" : `${format.integer(row.resultBytes)} B`}</td><td>{t(row.ok ? "mcp-result-ok" : "mcp-result-error")}</td>
      </tr>)}</tbody>
    </table></div>)}
    <div className="os-mcp-actions">
      <Button isDisabled={rows === null && !error} onPress={() => setReload(v => v + 1)}>{t("mcp-refresh")}</Button>
      <Button isDisabled={offset === 0 || rows === null} onPress={() => setOffset(n => Math.max(0, n - PAGE_SIZE))}>{t("mcp-previous")}</Button>
      <span>{t("mcp-page", { page: offset / PAGE_SIZE + 1 })}</span>
      <Button isDisabled={!rows || rows.length < PAGE_SIZE} onPress={() => setOffset(n => n + PAGE_SIZE)}>{t("mcp-next")}</Button>
    </div>
  </section>;
}
