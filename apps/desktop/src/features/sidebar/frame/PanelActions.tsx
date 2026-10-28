import { useState } from "react";
import { useT } from "../../../i18n";
import type { PanelKind } from "../../../ipc/gen/PanelKind";
import { openExport } from "../../export/model";
import { QueryFeedback } from "./QueryFeedback";

export function tsv(rows: readonly (readonly (string | number)[])[]): string {
  return rows.map(row => row.map(cell => String(cell).replace(/[\t\r\n]+/g, " ")).join("\t")).join("\n");
}

export function PanelActions({ which, rows }: { which: PanelKind; rows: (string | number)[][] }) {
  const t = useT();
  const [error, setError] = useState<unknown>();
  async function copy() {
    try { await navigator.clipboard.writeText(tsv(rows)); setError(undefined); } catch (e) { setError(e); }
  }
  // UI-060: every sidebar table is exportable through the shared export dialog (format, file name, status).
  const save = () => openExport(which === "issues" ? { kind: "issues", which } : { kind: "sidebar", which });
  return <>
    <div className="os-sidebar-actions" data-sidebar-export-which={which}><button type="button" onClick={() => void copy()}>{t("sidebar-copy")}</button><button type="button" onClick={save}>{t("sidebar-export")}</button></div>
    <QueryFeedback error={error} />
  </>;
}
