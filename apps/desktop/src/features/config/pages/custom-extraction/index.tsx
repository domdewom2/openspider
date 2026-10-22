import { useT } from "../../../../i18n";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { Action, Choice, Input, RowActions, RowIssues, coreGuard, moveRow } from "../custom-search/shared";
export interface Extractor { name: string; mode: string; expression: string; attribute: string | null; output: string }
export function changeMode(row: Extractor, mode: string): Extractor { return { ...row, mode, attribute: mode === "css" ? row.attribute : null, output: mode !== "xpath" && row.output === "function_value" ? "text" : row.output }; }
function Extraction() {
  const t = useT(); const { value = [], set, lock } = useConfigValue<Extractor[]>("custom_extraction.extractors"); const disabled = !!lock;
  const update = (i: number, p: Partial<Extractor>) => set(value.map((r, n) => n === i ? { ...r, ...p } : r));
  return <FieldFrame path="custom_extraction.extractors" showIssues={false}>{value.map((r, i) => <fieldset className="os-cfg-group" key={i}><legend>{r.name || `Extractor ${i + 1}`}</legend>
    <Input label={t("fach-name")} value={r.name} onChange={name => update(i, { name: name || `Extractor ${i + 1}` })} disabled={disabled} />
    <Choice label={t("fach-mode")} value={r.mode} options={["xpath", "css", "regex"]} onChange={mode => update(i, changeMode(r, mode))} disabled={disabled} />
    <Input multiline path={`custom_extraction.extractors[${i}].expression`} label={t("fach-expression")} value={r.expression} onChange={expression => update(i, { expression })} disabled={disabled} />
    {r.mode === "css" && <Input label={t("fach-attribute")} value={r.attribute ?? ""} onChange={attribute => update(i, { attribute: attribute || null })} disabled={disabled} />}
    {r.mode !== "regex" && <Choice label={t("fach-output")} value={r.output} options={r.mode === "xpath" ? ["text", "html_element", "inner_html", "function_value"] : ["text", "html_element", "inner_html"]} onChange={output => update(i, { output })} disabled={disabled || (r.mode === "css" && !!r.attribute)} />}
    {r.output === "function_value" && <p className="os-hint">{t("fach-function-hint")}</p>}
    <RowIssues path={`custom_extraction.extractors[${i}]`} syntax />
    <RowActions index={i} count={value.length} disabled={disabled} remove={() => set(value.filter((_, n) => n !== i))} move={to => set(moveRow(value, i, to))} />
  </fieldset>)}<Action disabled={disabled || value.length >= 100} onClick={() => set([...value, { name: `Extractor ${value.length + 1}`, mode: "xpath", expression: "", attribute: null, output: "text" }])}>{t("fach-add-extractor")}</Action>
  {value.length >= 100 && <p role="status">{t("fach-limit", { n: 100 })}</p>}<RowIssues path="custom_extraction.extractors" exact /></FieldFrame>;
}
export const page = definePage({ id: "custom-extraction", group: "custom", order: 110, anchors: ["custom_extraction"], fields: [f("custom_extraction.extractors", { hidden: true, labelKey: "fach-extractors" })], After: Extraction, saveGuard: coreGuard("custom_extraction.extractors") });
