import { catalog } from "../../../../ipc/catalog";
import { exportJson, pendingColumns } from "../../../segments-dialog/model";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useT } from "../../../../i18n";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { previewRevision, useDraftPreview } from "./preview";
import { useSession } from "../../../../state/session";
import { SEGMENT_PALETTE } from "../../../../ui/tokens/tokens";
import { Action, Choice, Input, RowActions, RowIssues, coreGuard, moveRow } from "../custom-search/shared";
import "./segments.css";
import { OPS, emptyCondition, importSegments, previewDefs, type Condition, type Segment } from "./model";
function Segments() {
  const t = useT(); const { value = [], set, lock } = useConfigValue<Segment[]>("segments.list"); const disabled = !!lock;
  const defs = useMemo(() => previewDefs(value), [value]); const hasCrawl = useSession(s => !!s.crawl?.crawlId);
  const preview = useDraftPreview(defs, hasCrawl);
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [columns, setColumns] = useState<string[]>([]);
  const revision = useSession(previewRevision);
  useEffect(() => { let live = true; void catalog.columns({ tab: "Internal" }).then(cols => { if (live) { setPending(pendingColumns(cols)); setColumns(cols.map(c => c.key)); } }).catch(() => undefined); return () => { live = false; }; }, [revision]);
  const [json, setJson] = useState(""); const [error, setError] = useState("");
  const jsonId = useId(); const fileInput = useRef<HTMLInputElement>(null); const [fileName, setFileName] = useState("");
  const update = (i: number, p: Partial<Segment>) => set(value.map((r, n) => n === i ? { ...r, ...p } : r));
  const patch = (i: number, g: number, c: number, p: Partial<Condition>) => update(i, { groups: value[i]!.groups.map((group, gi) => gi === g ? { conditions: group.conditions.map((cond, ci) => ci === c ? { ...cond, ...p } : cond) } : group) });
  return <FieldFrame path="segments.list" showIssues={false}><p className="os-hint">{t("fach-segment-hint")}</p><datalist id="fach-segment-columns">{columns.map(c => <option key={c} value={c} />)}</datalist>
    {value.map((s, i) => <fieldset key={i} className="os-cfg-group"><legend>{s.name || t("fach-segment")}</legend>
      <Input label={t("fach-name")} value={s.name} onChange={name => update(i, { name })} disabled={disabled} />
      <label>{t("fach-color")}<input type="color" value={/^#[0-9a-fA-F]{6}$/.test(s.color) ? s.color : "#000000"} onChange={e => update(i, { color: e.target.value })} disabled={disabled} list={`fach-colors-${i}`} /></label>
      <datalist id={`fach-colors-${i}`}>{SEGMENT_PALETTE.map(c => <option key={c.light} value={c.light} />)}</datalist>
      {s.groups.map((group, g) => <fieldset key={g} className="os-cfg-group"><legend>{t("fach-or-group", { n: g + 1 })}</legend>
        {group.conditions.map((c, ci) => <div className="os-cfg-field" key={ci}>
          <Input label={t("fach-field")} list="fach-segment-columns" value={c.field} onChange={field => patch(i, g, ci, { field })} disabled={disabled} />
          <Choice label={t("fach-operator")} value={c.op} options={OPS} onChange={op => patch(i, g, ci, { op })} disabled={disabled} />
          {!["is_empty", "is_not_empty", "in_no_segment"].includes(c.op) && (c.op === "in_segment" ? <label>{t("fach-value")}<select className="os-input" value={c.value} disabled={disabled} onChange={e => patch(i, g, ci, { value: e.target.value })}><option value="">{t("fach-pick-segment")}</option>{value.slice(0, i).map((above, ai) => <option key={ai} value={above.name}>{above.name}</option>)}</select></label> : <Input path={`segments.list[${i}].groups[${g}].conditions[${ci}].value`} label={t("fach-value")} value={c.value} onChange={v => patch(i, g, ci, { value: v })} disabled={disabled} />)}
          {pending.has(c.field) && <span className="os-hint">{t("sgd-pending")}</span>}
          <RowIssues path={`segments.list[${i}].groups[${g}].conditions[${ci}]`} />
          <Action disabled={disabled} onClick={() => update(i, { groups: s.groups.map((grp, gi) => gi === g ? { conditions: grp.conditions.filter((_, n) => n !== ci) } : grp) })}>{t("fach-remove-condition")}</Action>
        </div>)}<Action disabled={disabled} onClick={() => update(i, { groups: s.groups.map((grp, gi) => gi === g ? { conditions: [...grp.conditions, emptyCondition()] } : grp) })}>{t("fach-and-condition")}</Action>
        <Action disabled={disabled} onClick={() => update(i, { groups: s.groups.filter((_, n) => n !== g) })}>{t("fach-remove-group")}</Action>
      </fieldset>)}<Action disabled={disabled} onClick={() => update(i, { groups: [...s.groups, { conditions: [emptyCondition()] }] })}>{t("fach-or-add")}</Action>
      <p role="status">{t("fach-matches", { n: preview.counts.get(`draft-${i}`) ?? "…" })}</p>
      <RowIssues path={`segments.list[${i}]`} />
      <RowActions index={i} count={value.length} disabled={disabled} move={to => set(moveRow(value, i, to))} remove={() => set(value.filter((_, n) => n !== i))} />
    </fieldset>)}<Action disabled={disabled} onClick={() => set([...value, { name: "", color: SEGMENT_PALETTE[value.length % SEGMENT_PALETTE.length]!.light, groups: [{ conditions: [emptyCondition()] }] }])}>{t("fach-add-segment")}</Action>
    {preview.resultRevision !== undefined && <p role="status" className="os-hint">{t("fach-preview-revision", { revision: preview.countsVersion ?? 0 })}{preview.stale && ` · ${t("fach-preview-stale")}`}</p>}
    {preview.busy && <p role="status">{t("fach-checking")}</p>}{preview.error != null && <p role="alert" className="os-error">{t("fach-preview-error")}</p>}
    {/* A02-19: the JSON field spans the page below its label; the file choice uses own, translated controls instead of the
        browser's file input (which shows its own language). */}
    <div className="os-cfg-field os-segments-json">
      <label className="os-label" htmlFor={jsonId}>{t("fach-segments-json")}</label>
      <textarea id={jsonId} className="os-input os-cfg-textarea os-mono" rows={8} value={json} onChange={e => setJson(e.target.value)} disabled={disabled} spellCheck={false} />
      <div className="os-segments-actions">
        <Action onClick={() => setJson(exportJson(previewDefs(value)))}>{t("fach-export-json")}</Action>
        <Action disabled={disabled} onClick={() => { try { set(importSegments(json)); setError(""); } catch { setError(t("fach-json-error")); } }}>{t("fach-import")}</Action>
      </div>
    </div>
    <div className="os-cfg-field os-segments-file" role="group" aria-labelledby={`${jsonId}-file`}>
      <span className="os-label" id={`${jsonId}-file`}>{t("fach-import-file")}</span>
      <input ref={fileInput} type="file" accept=".json,application/json" hidden disabled={disabled} onChange={e => { const file = e.target.files?.[0]; e.target.value = ""; if (file) { setFileName(file.name); void file.text().then(text => { set(importSegments(text)); setError(""); }).catch(() => setError(t("fach-json-error"))); } }} />
      <div className="os-segments-actions">
        <Action disabled={disabled} onClick={() => fileInput.current?.click()}>{t("fach-choose-file")}</Action>
        <span className="os-hint" role="status">{fileName || t("fach-no-file")}</span>
      </div>
    </div>
    {error && <p role="alert" className="os-error">{error}</p>}<RowIssues path="segments.list" exact />
  </FieldFrame>;
}
export const page = definePage({ id: "segments", order: 160, anchors: ["segments"], fields: [f("segments.list", { hidden: true, labelKey: "fach-segments" })], After: Segments, saveGuard: coreGuard("segments.list") });
