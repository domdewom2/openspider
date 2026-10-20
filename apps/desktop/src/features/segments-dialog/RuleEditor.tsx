import { useId } from "react";
import { useT } from "../../i18n";
import type { SegmentDef } from "../../ipc/gen/SegmentDef";
import type { ValidationIssue } from "../../ipc/gen/ValidationIssue";
import { NO_VALUE, OPS, conditionError, emptyCondition, fromGroups, issueTarget, toGroups, type Condition, type FieldError } from "./model";

export interface RuleEditorProps {
  list: readonly SegmentDef[];
  index: number;
  issues: readonly ValidationIssue[];
  columns: readonly string[];
  pending: ReadonlySet<string>;
  onChange: (rules: SegmentDef["rules"]) => void;
}

/** Rule editor of one segment: groups are ORed, the conditions inside a group are ANDed (CRAWL-134, UI-068). */
export function RuleEditor({ list, index, issues, columns, pending, onChange }: RuleEditorProps) {
  const t = useT();
  const listId = useId();
  const segment = list[index]!;
  const groups = toGroups(segment.rules);
  const set = (next: Condition[][]) => onChange(fromGroups(next));
  const patch = (g: number, c: number, p: Partial<Condition>) => set(groups.map((grp, gi) => gi === g ? grp.map((cond, ci) => ci === c ? { ...cond, ...p } : cond) : grp));
  const coreIssues = (g: number, c: number) => issues.filter(i => { const x = issueTarget(i.path); return x.kind === "condition" && x.group === g && x.condition === c; });
  const text = (e: FieldError) => t(e.key, e.args);
  return <div className="os-segdlg-rules">
    <datalist id={listId}>{columns.map(c => <option key={c} value={c} />)}</datalist>
    {groups.map((group, g) => <section key={g} className="os-segdlg-group" aria-label={t("sgd-group", { n: g + 1 })}>
      {g > 0 && <div className="os-segdlg-or" aria-hidden="true">{t("sgd-or")}</div>}
      <h4>{t("sgd-group", { n: g + 1 })}</h4>
      <ul>{group.map((cond, c) => {
        const local = conditionError(list, index, cond);
        const core = coreIssues(g, c);
        const errors = [...(local ? [text(local)] : []), ...core.filter(i => !i.warning).map(i => t(i.messageKey, i.args as Record<string, string>) + (i.line != null ? ` (${t("sgd-position", { line: i.line, column: i.column ?? 0 })})` : ""))];
        const hints = core.filter(i => i.warning).map(i => t(i.messageKey, i.args as Record<string, string>));
        const waiting = !!cond.column && pending.has(cond.column);
        const errId = `${listId}-${g}-${c}`;
        return <li key={c} className="os-segdlg-cond" role="group" aria-label={t("sgd-condition", { g: g + 1, c: c + 1 })}>
          <input className="os-input" list={listId} aria-label={t("sgd-column")} placeholder={t("sgd-any-column")} value={cond.column ?? ""} onChange={e => patch(g, c, { column: e.target.value === "" ? null : e.target.value })} />
          <select className="os-input" aria-label={t("sgd-operator")} value={cond.op} onChange={e => patch(g, c, { op: e.target.value as Condition["op"] })}>{OPS.map(op => <option key={op} value={op}>{t(`sgd-op-${op}`)}</option>)}</select>
          {!NO_VALUE.includes(cond.op) && <input className="os-input" aria-label={t("sgd-value")} aria-invalid={errors.length > 0 || undefined} aria-describedby={errors.length > 0 ? errId : undefined} value={cond.value ?? ""} onChange={e => patch(g, c, { value: e.target.value })} />}
          {waiting && <span className="os-segdlg-pending" title={t("sgd-pending-hint")}>{t("sgd-pending")}</span>}
          <button type="button" className="os-btn os-btn-tool" aria-label={t("sgd-remove-condition", { g: g + 1, c: c + 1 })} disabled={group.length === 1 && groups.length === 1} onClick={() => set(group.length === 1 ? groups.filter((_, gi) => gi !== g) : groups.map((grp, gi) => gi === g ? grp.filter((_, ci) => ci !== c) : grp))}>×</button>
          {(errors.length > 0 || hints.length > 0) && <div id={errId} className="os-segdlg-issues">{errors.map((m, i) => <p key={`e${i}`} className="os-error" role="alert" data-segdlg-error>{m}</p>)}{hints.map((m, i) => <p key={`w${i}`} className="os-hint">{m}</p>)}</div>}
        </li>;
      })}</ul>
      <button type="button" className="os-btn os-btn-secondary" onClick={() => set(groups.map((grp, gi) => gi === g ? [...grp, emptyCondition()] : grp))}>{t("sgd-add-condition")}</button>
    </section>)}
    <button type="button" className="os-btn os-btn-secondary" onClick={() => set([...groups, [emptyCondition()]])}>{t("sgd-add-group")}</button>
  </div>;
}
