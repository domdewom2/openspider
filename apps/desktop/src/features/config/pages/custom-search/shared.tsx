import { useId, type ReactNode } from "react";
import { useT } from "../../../../i18n";
import { issueText, useConfig } from "../../frame/context";
import { getPath, issuesFor } from "../../frame/model";
import type { ConfigDraft } from "../../frame/draft";

export function Input({ label, value, onChange, disabled = false, multiline = false, list, path }: { label: string; list?: string; path?: string; value: string; onChange(v: string): void; disabled?: boolean; multiline?: boolean }) {
  const id = useId();
  const t = useT(); const { draft } = useConfig();
  const errors = path ? issuesFor(draft.issues, path).filter(i => !i.warning) : [];
  const validation = { "aria-invalid": errors.length > 0 || undefined, title: errors.length ? errors.map(i => issueText(t, i)).join("; ") : undefined };
  return <label className="os-label" htmlFor={id}>{label}{multiline
    ? <textarea {...validation} id={id} className="os-input os-cfg-textarea os-mono" rows={4} value={value} onChange={e => onChange(e.target.value)} disabled={disabled} spellCheck={false} />
    : <input {...validation} id={id} list={list} className="os-input" value={value} onChange={e => onChange(e.target.value)} disabled={disabled} />}</label>;
}
export function Choice({ label, value, options, onChange, disabled = false }: { label: string; value: string; options: readonly string[]; onChange(v: string): void; disabled?: boolean }) {
  const t = useT(); const id = useId();
  return <div className="os-cfg-field"><label className="os-label" htmlFor={id}>{label}</label><select id={id} className="os-input" value={value} onChange={e => onChange(e.target.value)} disabled={disabled}>{options.map(v => <option key={v} value={v}>{t(`fach-option-${v.replaceAll("_", "-")}`)}</option>)}</select></div>;
}
export function Action({ children, onClick, disabled = false }: { children: ReactNode; onClick(): void; disabled?: boolean }) {
  return <button type="button" className="os-btn os-btn-secondary" onClick={onClick} disabled={disabled}>{children}</button>;
}
export function RowActions({ index, count, remove, move, disabled, lastFixed = false }: { index: number; count: number; remove(): void; move(to: number): void; disabled: boolean; lastFixed?: boolean }) {
  const t = useT();
  return <div className="os-cfg-inline"><Action disabled={disabled || index === 0 || (lastFixed && index === count - 1)} onClick={() => move(index - 1)}>{t("fach-up")}</Action><Action disabled={disabled || index >= count - (lastFixed ? 2 : 1)} onClick={() => move(index + 1)}>{t("fach-down")}</Action><Action disabled={disabled || (lastFixed && index === count - 1)} onClick={remove}>{t("fach-remove")}</Action></div>;
}
export function moveRow<T>(rows: T[], from: number, to: number): T[] {
  if (to < 0 || to >= rows.length) return rows;
  const next = [...rows]; const [row] = next.splice(from, 1); next.splice(to, 0, row!); return next;
}
export function RowIssues({ path, syntax = false, exact = false }: { path: string; syntax?: boolean; exact?: boolean }) {
  const { draft } = useConfig(); const t = useT();
  const errors = issuesFor(draft.issues, path).filter(i => !exact || i.path === path);
  const checked = JSON.stringify(draft.validatedConfig) === JSON.stringify(draft.cfg);
  return <div aria-live="polite">{syntax && <span className={errors.some(i => !i.warning) ? "os-error" : "os-hint"} title={errors.map(i => issueText(t, i) + (i.column == null ? "" : ` (${i.line ?? 1}:${i.column})`)).join("; ")}>{t(!checked ? "fach-checking" : errors.some(i => !i.warning) ? "fach-invalid" : "fach-valid")}</span>}{errors.map((i, n) => <p key={n} className={i.warning ? "os-hint" : "os-error"} role="alert">{issueText(t, i)}{i.column != null && ` (${i.line ?? 1}:${i.column})`}</p>)}</div>;
}
/** Save only a configuration that the core has checked; XPath/regex never use the browser dialect. */
export function coreGuard(path: string, active: (draft: ConfigDraft) => boolean = d => (d.get<unknown[]>(path)?.length ?? 0) > 0) {
  return (draft: ConfigDraft): boolean => active(draft) && (JSON.stringify(getPath(draft.validatedConfig, path.split(".")[0]!)) !== JSON.stringify(getPath(draft.cfg, path.split(".")[0]!)) || draft.issues.some(i => !i.warning && (i.path === "" || i.path.startsWith(path.split(".")[0]!))));
}
