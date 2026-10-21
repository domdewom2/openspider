import { useEffect, useState } from "react";
import { Banner } from "../../../../ui/primitives";
import { useT } from "../../../../i18n";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { Action, Choice, Input, RowActions, RowIssues, coreGuard, moveRow } from "../custom-search/shared";
interface Snippet { name: string; kind: string; code: string; timeout_s: number; content_types: string[] }
const parseMime = (text: string) => text.split(",").map(s => s.trim()).filter(Boolean);
function MimeTypes({ value, set, disabled }: { value: string[]; set(v: string[]): void; disabled: boolean }) {
  const t = useT();
  const [text, setText] = useState(value.join(", "));
  const key = JSON.stringify(value);
  useEffect(() => {
    if (JSON.stringify(parseMime(text)) !== key) setText(value.join(", "));
    // Keep a trailing comma while the user is typing; only external changes replace the text.
  }, [key]);
  return <Input label={t("fach-mime")} value={text} onChange={v => { setText(v); set(parseMime(v)); }} disabled={disabled} />;
}
function Snippets() {
  const t = useT(); const rendering = useConfigValue<string>("rendering.mode").value;
  const { value = [], set, lock } = useConfigValue<Snippet[]>("custom_js.snippets"); const disabled = !!lock;
  const update = (i: number, p: Partial<Snippet>) => set(value.map((r, n) => n === i ? { ...r, ...p } : r));
  return <FieldFrame path="custom_js.snippets" showIssues={false}>{rendering !== "javascript" && <Banner kind="warning">{t("fach-js-rendering")}</Banner>}
    {value.map((r, i) => <fieldset key={i} className="os-cfg-group"><legend>{r.name || `Snippet ${i + 1}`}</legend>
      <Input label={t("fach-name")} value={r.name} onChange={name => update(i, { name: name || `Snippet ${i + 1}` })} disabled={disabled} />
      <Choice label={t("fach-type")} value={r.kind} options={["extraction", "action"]} onChange={kind => update(i, { kind })} disabled={disabled} />
      <MimeTypes value={r.content_types} set={content_types => update(i, { content_types })} disabled={disabled} />
      <label className="os-label">{t("fach-timeout")}<input className="os-input" type="number" min={r.kind === "action" ? 1 : 0} max={r.kind === "action" ? 120 : undefined} value={r.timeout_s} disabled={disabled} onChange={e => update(i, { timeout_s: e.target.value === "" ? 0 : Number(e.target.value) })} /></label>
      <details open><summary>{t("fach-js-editor")}</summary><Input multiline label={t("fach-js-code")} value={r.code} onChange={code => update(i, { code })} disabled={disabled} /></details>
      <RowIssues path={`custom_js.snippets[${i}]`} />
      <RowActions index={i} count={value.length} disabled={disabled} remove={() => set(value.filter((_, n) => n !== i))} move={to => set(moveRow(value, i, to))} />
    </fieldset>)}<Action disabled={disabled || value.length >= 100} onClick={() => set([...value, { name: `Snippet ${value.length + 1}`, kind: "extraction", code: "", content_types: ["text/html"], timeout_s: 10 }])}>{t("fach-add-snippet")}</Action>
    {value.length >= 100 && <p role="status">{t("fach-limit", { n: 100 })}</p>}<RowIssues path="custom_js.snippets" exact />
  </FieldFrame>;
}
export const page = definePage({ id: "custom-js", group: "custom", order: 120, anchors: ["custom_js"], fields: [f("custom_js.snippets", { hidden: true, labelKey: "fach-snippets" })], After: Snippets, saveGuard: coreGuard("custom_js.snippets") });
