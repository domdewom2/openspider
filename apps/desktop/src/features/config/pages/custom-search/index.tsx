import { Dialog } from "../../../../ui/primitives";
import { useState } from "react";
import { useT } from "../../../../i18n";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { Action, Choice, Input, RowActions, RowIssues, coreGuard, moveRow } from "./shared";
import { SEARCH_SCOPES, bulkAdd, importExpressions, newFilter, type SearchFilter } from "./model";

function Options({ row, patch, disabled }: { row: SearchFilter; patch(p: Partial<SearchFilter>): void; disabled: boolean }) {
  const t = useT();
  return <><Choice label={t("fach-condition")} value={String(row.contains)} options={["true", "false"]} onChange={v => patch({ contains: v === "true" })} disabled={disabled} />
    <Choice label={t("fach-type")} value={row.kind} options={["text", "regex"]} onChange={kind => patch({ kind })} disabled={disabled} />
    <Choice label={t("fach-scope")} value={row.scope} options={SEARCH_SCOPES} onChange={scope => patch({ scope })} disabled={disabled} />
    {row.scope === "xpath" && <Input label="XPath" value={row.xpath ?? ""} onChange={xpath => patch({ xpath })} disabled={disabled} />}
    {row.kind === "text" && <label><input type="checkbox" checked={row.case_sensitive} onChange={e => patch({ case_sensitive: e.target.checked })} disabled={disabled} />{t("fach-case")}</label>}</>;
}
function Search() {
  const t = useT(); const { value = [], set, lock } = useConfigValue<SearchFilter[]>("custom_search.filters"); const disabled = !!lock;
  const [bulk, showBulk] = useState(false); const [text, setText] = useState(""); const [template, setTemplate] = useState(newFilter(1)); const [notice, setNotice] = useState("");
  const update = (i: number, p: Partial<SearchFilter>) => set(value.map((r, n) => n === i ? { ...r, ...p } : r));
  return <FieldFrame path="custom_search.filters" showIssues={false}>
    {value.map((row, i) => <fieldset className="os-cfg-group" key={i}><legend>{row.name || `Filter ${i + 1}`}</legend>
      <Input label={t("fach-name")} value={row.name} onChange={name => update(i, { name: name || `Filter ${i + 1}` })} disabled={disabled} />
      <Options row={row} patch={p => update(i, p)} disabled={disabled} />
      <Input multiline path={`custom_search.filters[${i}].expression`} label={t("fach-expression")} value={row.expression} onChange={expression => update(i, { expression })} disabled={disabled} />
      {row.kind === "regex" && <p className="os-hint">{t("fach-regex-hint")}</p>}
      <RowIssues path={`custom_search.filters[${i}]`} syntax />
      <RowActions index={i} count={value.length} disabled={disabled} remove={() => set(value.filter((_, n) => n !== i))} move={to => set(moveRow(value, i, to))} />
    </fieldset>)}
    <Action disabled={disabled || value.length >= 100} onClick={() => set([...value, newFilter(value.length + 1)])}>{t("fach-add-filter")}</Action>
    {value.length >= 100 && <p role="status">{t("fach-limit", { n: 100 })}</p>}
    <Action disabled={disabled} onClick={() => showBulk(!bulk)}>{t("fach-bulk")}</Action>
    {bulk && <Dialog title={t("fach-bulk")} isOpen onOpenChange={showBulk}><fieldset className="os-cfg-group"><legend>{t("fach-bulk")}</legend><Options row={template} patch={p => setTemplate({ ...template, ...p })} disabled={disabled} />
      <Input multiline label={t("fach-bulk-lines")} value={text} onChange={setText} disabled={disabled} />
      <label>{t("fach-import-file")}<input type="file" accept=".txt,.csv,text/plain,text/csv" disabled={disabled} onChange={e => { const file = e.target.files?.[0]; if (file) void file.text().then(setText).catch(() => setNotice(t("fach-file-error"))); }} /></label>
      <Action disabled={disabled} onClick={() => { const r = bulkAdd(value, importExpressions(text), template); set(r.rows); setNotice(t("fach-bulk-result", { skipped: r.skipped, overflow: r.overflow })); showBulk(false); }}>{t("fach-import")}</Action>
      <Action onClick={() => showBulk(false)}>{t("config-cancel")}</Action>
    </fieldset></Dialog>}{notice && <p role="status">{notice}</p>}
    <RowIssues path="custom_search.filters" exact />
  </FieldFrame>;
}
export const page = definePage({ id: "custom-search", group: "custom", order: 100, anchors: ["custom_search"], fields: [f("custom_search.filters", { hidden: true, labelKey: "fach-filters" }), f("custom_search.regex_timeout_ms", { labelKey: "fach-regex-timeout" })], After: Search, saveGuard: coreGuard("custom_search.filters") });
