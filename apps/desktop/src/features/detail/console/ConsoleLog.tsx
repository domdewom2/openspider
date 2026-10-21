import { useState } from "react";
import { exportApi } from "../../../ipc/export";
import type { ExportItem } from "../../../ipc/gen/ExportItem";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { Select } from "../../../ui/primitives/fields";
import { openExport } from "../../export/model";
import { useDetailRequest } from "../frame/request";
import { DetailLoading } from "../frame/Feedback";
import { TablePanel } from "../frame/TablePanel";
import type { DetailPanelProps } from "../frame/types";
import { copyText, FachEmpty, featureDisabled, focusOf, gate, needsRendering, useFachConfig } from "../rendered/common";
import { CONSOLE_COLUMNS, CONSOLE_LEVELS, findBulk, findReport, shortenMessage } from "./model";

export function ConsoleLog(props: DetailPanelProps) {
  const t = useT(); const cfg = useFachConfig(props.selection.crawlId); const { actions } = props; const focus = focusOf(props.selection);
  const [level, setLevel] = useState("all"); const [sort, setSort] = useState<SortKey[]>([]);
  const [message, setMessage] = useState<{ level: string; text: string }>();
  const rendering = props.rendering || cfg.rendering;
  const empty = cfg.loaded && gate(!rendering && needsRendering(), rendering && !cfg.jsErrors && featureDisabled("rendering.js_error_reporting"));
  const catalog = useDetailRequest(`console-catalog:${props.selection.crawlId}`, signal => exportApi.catalog(signal));
  const items = (groups: { items: ExportItem[] }[] | undefined) => groups?.flatMap(group => group.items) ?? [];
  const bulkItems = items(catalog.value?.bulk); const reportItems = items(catalog.value?.reports);
  const bulkName = findBulk(bulkItems.map(i => i.name)); const reportName = findReport(reportItems.map(i => i.name));
  const bulk = bulkItems.find(i => i.name === bulkName); const report = reportItems.find(i => i.name === reportName);
  const reason = (item: ExportItem | undefined) => !rendering || !cfg.jsErrors ? t("console-export-needs-capture") : !item ? t("console-export-unavailable") : item.enabled ? undefined : item.disabledReason ? t(item.disabledReason) : t("console-export-needs-capture");
  if (!cfg.loaded) return <DetailLoading />;
  if (empty) return <div className="os-fach"><FachEmpty state={empty} onError={actions.reportError} /></div>;
  const links: [string, string, ExportItem | undefined, "bulk" | "report"][] = [
    ["bulk", t("console-export-pages"), bulk, "bulk"], ["report", t("console-export-summary"), report, "report"]];
  const shown = message ? shortenMessage(message.text) : undefined;
  return <div className="os-fach os-console">
    <div className="os-fach-toolbar">
      <Select label={t("console-level")} value={level} onChange={setLevel}
        options={[{ value: "all", label: t("console-level-all") }, ...CONSOLE_LEVELS.map(value => ({ value, label: t(`console-level-${value}`) }))]} />
      {links.map(([id, label, item, kind]) => {
        const why = reason(item);
        return <span key={id} title={why}><Button isDisabled={!!why} onPress={() => { if (item) openExport({ kind, name: item.name }); }}>{label}</Button></span>;
      })}
    </div>
    <div className="os-fach-fill">
      <TablePanel {...props} title={t("detail-tab-consoleLog")} columns={CONSOLE_COLUMNS} plainContext
        spec={{ kind: "detail", detail: "consoleLog", urlIds: [focus], params: level === "all" ? {} : { level }, sort }} onSort={setSort}
        onSelectRow={get => setMessage({ level: get("Level"), text: get("Message") })} />
    </div>
    {message && shown && <section className="os-fach-pane os-console-message" aria-label={t("console-message")}>
      <div className="os-fach-actions"><h3>{t("console-message")}</h3><Button onPress={() => copyText(message.text, actions.reportError)}>{t("console-copy")}</Button></div>
      <pre className="os-fach-code os-console-text">{shown.text}</pre>
      {shown.truncated && <p className="os-fach-note" role="status">{t("console-truncated", { total: shown.total })}</p>}
    </section>}
  </div>;
}
