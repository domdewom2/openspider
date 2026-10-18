import { useState } from "react";
import { useT } from "../../../i18n";
import { exportApi } from "../../../ipc/export";
import type { PanelKind } from "../../../ipc/gen/PanelKind";
import type { ExportFormatDto } from "../../../ipc/gen/ExportFormatDto";
import { Dialog, DialogFooter } from "../../../ui/primitives/Dialog";
import { QueryFeedback } from "./QueryFeedback";

export function tsv(rows: readonly (readonly (string | number)[])[]): string {
  return rows.map(row => row.map(cell => String(cell).replace(/[\t\r\n]+/g, " ")).join("\t")).join("\n");
}

export function PanelActions({ which, rows }: { which: PanelKind; rows: (string | number)[][] }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");
  const [format, setFormat] = useState<ExportFormatDto>("csv");
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  async function copy() {
    try { await navigator.clipboard.writeText(tsv(rows)); setError(undefined); } catch (e) { setError(e); }
  }
  async function save() {
    setBusy(true);
    setError(undefined);
    try {
      const args = { which, path: path.trim(), format };
      await (which === "issues" ? exportApi.issues(args) : exportApi.sidebar(args));
      setOpen(false);
    } catch (e) { setError(e); } finally { setBusy(false); }
  }
  return <>
    <div className="os-sidebar-actions"><button type="button" onClick={() => void copy()}>{t("sidebar-copy")}</button><button type="button" onClick={() => setOpen(true)}>{t("sidebar-export")}</button></div>
    {!open && <QueryFeedback error={error} />}
    <Dialog title={t("sidebar-export")} isOpen={open} onOpenChange={setOpen} footer={<DialogFooter okLabel={t("sidebar-export")} cancelLabel={t("sidebar-cancel")} onCancel={() => setOpen(false)} onOk={() => void save()} okDisabled={busy || !path.trim()} />}>
      <label>{t("sidebar-path")}<input value={path} onChange={e => setPath(e.target.value)} /></label>
      <label>{t("sidebar-format")}<select aria-label={t("sidebar-format")} value={format} onChange={e => setFormat(e.target.value as ExportFormatDto)}><option value="csv">CSV</option><option value="xlsx">XLSX</option></select></label>
      <QueryFeedback error={error} loading={busy} />
    </Dialog>
  </>;
}
