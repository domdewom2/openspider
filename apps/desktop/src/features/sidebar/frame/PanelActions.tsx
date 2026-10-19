import { useState } from "react";
import { useT } from "../../../i18n";
import type { PanelKind } from "../../../ipc/gen/PanelKind";
import { runAction } from "../../../ui/actions";
import { QueryFeedback } from "./QueryFeedback";

export function tsv(rows: readonly (readonly (string | number)[])[]): string {
  return rows.map(row => row.map(cell => String(cell).replace(/[\t\r\n]+/g, " ")).join("\t")).join("\n");
}

export function PanelActions({ which, rows }: { which: PanelKind; rows: (string | number)[][] }) {
  const t = useT();
  const [error, setError] = useState<unknown>();
  const [exportUnavailable, setExportUnavailable] = useState(false);
  async function copy() {
    try { await navigator.clipboard.writeText(tsv(rows)); setError(undefined); } catch (e) { setError(e); }
  }
  function save() {
    // STUB(M2-N01): U16 is absent on this branch. Register the active panel via
    // registerExportContext("sidebar", …) when integrating U16's native dialog.
    setExportUnavailable(!runAction("export.sidebar"));
  }
  return <>
    <div className="os-sidebar-actions" data-sidebar-export-which={which}><button type="button" onClick={() => void copy()}>{t("sidebar-copy")}</button><button type="button" onClick={save}>{t("sidebar-export")}</button></div>
    <QueryFeedback error={error} />
    {exportUnavailable && <p role="status">{t("sidebar-export-unavailable")}</p>}
  </>;
}
