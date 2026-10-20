import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useT } from "../../i18n";
import { tasks } from "../../ipc/tasks";
import { mail } from "../../ipc/mail";
import { auth } from "../../ipc/auth";
import type { AuthProfileInfo } from "../../ipc/gen/AuthProfileInfo";
import { exportApi } from "../../ipc/export";
import type { TaskDef } from "../../ipc/gen/TaskDef";
import type { ScheduleDef } from "../../ipc/gen/ScheduleDef";
import type { MailAccountInfo } from "../../ipc/gen/MailAccountInfo";
import type { ValidationIssue } from "../../ipc/gen/ValidationIssue";
import type { ExportMultiArgs } from "../../ipc/gen/ExportMultiArgs";
import type { ExportPreset } from "../../ipc/gen/ExportPreset";
import { Checkbox, Dialog, DialogFooter, Tabs, TextField } from "../../ui/primitives";
import { MultiOptions } from "../export/MultiOptions";
import { defaultSitemapOptions } from "../export/sitemap";
import { CustomSummary } from "./CustomSummary";
import { taskError } from "./model";

// A missing catalog must not prevent preserving existing export selections.
const ignoreCatalogReadiness = () => undefined;

export function TaskEditor({ initial, onClose, onSaved }: { initial: TaskDef; onClose(): void; onSaved(): void }) {
  const t = useT();
  const [task, setTask] = useState<TaskDef>(() => structuredClone(initial));
  const [tab, setTab] = useState("general");
  const [subtab, setSubtab] = useState("output");
  const [accounts, setAccounts] = useState<MailAccountInfo[]>([]);
  const [authProfiles, setAuthProfiles] = useState<AuthProfileInfo[]>([]);
  const authListId = useId();
  const [presets, setPresets] = useState<ExportPreset[]>([]);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [savedId, setSavedId] = useState<string | null>(null);
  const lock = useRef(false);
  const reportError = useCallback((e: unknown) => setError(taskError(e)), []);
  const patch = (next: Partial<TaskDef>) => { setTask(v => ({ ...v, ...next })); setIssues([]); };
  useEffect(() => {
    const abort = new AbortController();
    void Promise.allSettled([mail.listAccounts(abort.signal), exportApi.presetList(abort.signal), auth.listProfiles(abort.signal)]).then(([mailResult, presetResult, authResult]) => {
      if (abort.signal.aborted) return;
      if (mailResult.status === "fulfilled") setAccounts(mailResult.value);
      if (presetResult.status === "fulfilled") setPresets(presetResult.value);
      if (authResult.status === "fulfilled") setAuthProfiles(authResult.value);
      for (const result of [mailResult, presetResult, authResult]) if (result.status === "rejected") reportError(result.reason);
    });
    return () => abort.abort();
  }, []);
  const schedule = task.schedule;
  const scheduleError = schedule.kind === "once" && (!schedule.at || !Number.isFinite(Date.parse(schedule.at)) || Date.parse(schedule.at) <= Date.now()) ? t("tasks-once-future")
    : schedule.kind === "weekly" && !schedule.days.length ? t("tasks-weekday-required")
    : schedule.kind === "monthly" && (!Number.isInteger(schedule.day) || schedule.day < 1 || schedule.day > 31) ? t("tasks-month-day-invalid") : null;
  const output: ExportMultiArgs = task.exportOptions ?? { items: task.exports.filter(e => e.kind !== "sitemap").map(({ kind, name }) => ({ kind, name })), format: task.exports[0]?.format ?? "csv", dir: task.exports[0]?.dir ?? "", singleWorkbook: false, skipEmpty: true };
  const setOutput = (next: ExportMultiArgs) => patch({ exportOptions: next, exports: [...task.exports.filter(e => e.kind === "sitemap").map(e => ({ ...e, dir: next.dir || null })), ...next.items.filter(e => e.kind === "tab" || e.kind === "bulk" || e.kind === "report").map(e => ({ kind: e.kind as "tab" | "bulk" | "report", name: e.name, format: next.format, dir: next.dir || null }))] });
  const changeKind = (kind: ScheduleDef["kind"]) => {
    const time = "time" in schedule ? schedule.time : "03:00";
    patch({ schedule: kind === "once" ? { kind, at: "" } : kind === "weekly" ? { kind, time, days: [1] } : kind === "monthly" ? { kind, time, day: 1 } : { kind, time } });
  };
  const save = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try {
      // A registration failure leaves the saved draft editable and retries the same ID.
      const candidate: TaskDef = { ...task, id: savedId ?? task.id, name: task.name.trim(), project: task.project?.trim() || task.name.trim(), execution: { ...task.execution, headless: true }, notify: { ...task.notify, to: task.notify.to.map(address => address.trim()).filter(Boolean) } };
      const found = await tasks.validate({ task: candidate });
      setIssues(found);
      if (found.some(i => !i.warning)) return;
      const info = await tasks.save({ task: candidate });
      setSavedId(info.id);
      if (candidate.enabled && !info.registered) await tasks.register({ id: info.id, dryRun: false });
      if (!candidate.enabled && info.registered) await tasks.unregister({ id: info.id });
      onSaved();
    } catch (e) { reportError(e); }
    finally { lock.current = false; setBusy(false); }
  };
  const field = (key: string, value: string, onChange: (value: string) => void) => <TextField label={t(`tasks-${key}`)} value={value} onChange={onChange} />;
  const general = <>
    {field("name", task.name, name => patch({ name }))}
    {field("project", task.project ?? "", project => patch({ project }))}
    <Checkbox isSelected={task.enabled} onChange={enabled => patch({ enabled })}>{t("tasks-enabled")}</Checkbox>
    <label className="os-field">{t("tasks-interval")}<select value={schedule.kind} onChange={e => changeKind(e.target.value as ScheduleDef["kind"])}>
      {["once", "daily", "weekly", "monthly"].map(kind => <option key={kind} value={kind}>{t(`tasks-${kind}`)}</option>)}
    </select></label>
    {schedule.kind === "once" ? <label className="os-field">{t("tasks-start")}<input type="datetime-local" value={localDateInput(schedule.at)} onChange={e => patch({ schedule: { kind: "once", at: e.target.value ? localDateWithOffset(e.target.value) : "" } })} /></label> :
      <label className="os-field">{t("tasks-time")}<input type="time" value={schedule.time} onChange={e => patch({ schedule: { ...schedule, time: e.target.value } })} /></label>}
    {schedule.kind === "weekly" && <fieldset><legend>{t("tasks-weekdays")}</legend>{Array.from({ length: 7 }, (_, i) => i + 1).map(day => <Checkbox key={day} isSelected={schedule.days.includes(day)} onChange={checked => patch({ schedule: { ...schedule, days: checked ? [...schedule.days, day].sort() : schedule.days.filter(d => d !== day) } })}>{t(`tasks-day-${day}`)}</Checkbox>)}</fieldset>}
    {schedule.kind === "monthly" && <TextField type="number" label={t("tasks-month-day")} value={String(schedule.day)} onChange={v => patch({ schedule: { ...schedule, day: Number(v) } })} />}
    <p>{t("tasks-local-time")}</p>
    {scheduleError && <p role="alert">{scheduleError}</p>}
    <Checkbox isSelected={task.execution.runIfMissed} onChange={runIfMissed => patch({ execution: { ...task.execution, runIfMissed } })}>{t("tasks-catch-up")}</Checkbox>
  </>;
  const spider = <>
    <label className="os-field">{t("tasks-mode")}<select value={task.crawl.source.kind} onChange={e => patch({ crawl: { ...task.crawl, source: e.target.value === "list" ? { kind: "list", path: "" } : { kind: "spider", url: "" } } })}><option value="spider">Spider</option><option value="list">{t("tasks-list-mode")}</option></select></label>
    {task.crawl.source.kind === "spider" ? field("source", task.crawl.source.url, url => patch({ crawl: { ...task.crawl, source: { kind: "spider", url } } })) : field("source", task.crawl.source.path, path => patch({ crawl: { ...task.crawl, source: { kind: "list", path } } }))}
    {field("profile", task.crawl.profile ?? "", profile => patch({ crawl: { ...task.crawl, profile: profile || null } }))}
    <label className="os-field">{t("tasks-auth-profile")}<input list={authListId} value={task.crawl.authProfile ?? ""} onChange={e => patch({ crawl: { ...task.crawl, authProfile: e.target.value || null } })} /></label>
    <datalist id={authListId}>{authProfiles.map(profile => <option key={profile.name} value={profile.name}>{profile.host}</option>)}</datalist>
    <Checkbox isDisabled={!task.project?.trim()} isSelected={task.execution.autoCompare} onChange={autoCompare => patch({ execution: { ...task.execution, autoCompare } })}>{t("tasks-auto-compare")}</Checkbox>
    {task.id && <p>{t("tasks-next-run-note")}</p>}
  </>;
  const notifications = <>
    <Checkbox isSelected={task.notify.onSuccess} onChange={onSuccess => patch({ notify: { ...task.notify, onSuccess } })}>{t("tasks-notify-success")}</Checkbox>
    <Checkbox isSelected={task.notify.onFailure} onChange={onFailure => patch({ notify: { ...task.notify, onFailure } })}>{t("tasks-notify-failure")}</Checkbox>
    <label className="os-field">{t("tasks-mail-account")}<select value={task.notify.account ?? ""} onChange={e => patch({ notify: { ...task.notify, account: e.target.value || null } })}>
      <option value="">{t("tasks-none")}</option>{task.notify.account && !accounts.some(a => a.id === task.notify.account) && <option value={task.notify.account}>{task.notify.account}</option>}{accounts.map(a => <option key={a.id} value={a.id}>{a.label}</option>)}
    </select></label>
    {field("recipients", task.notify.to.join(", "), v => patch({ notify: { ...task.notify, to: v.split(/[,;\n]/).map(s => s.trim()) } }))}
  </>;
  const exportTabs = [
    { id: "output", label: t("tasks-output"), content: <>
      {field("directory", output.dir, dir => setOutput({ ...output, dir }))}
      <label className="os-field">{t("tasks-format")}<select value={output.format} onChange={e => setOutput({ ...output, format: e.target.value as "csv" | "xlsx", singleWorkbook: e.target.value === "xlsx" && output.singleWorkbook })}><option value="csv">CSV</option><option value="xlsx">XLSX</option></select></label>
      <Checkbox isSelected={output.singleWorkbook} isDisabled={output.format !== "xlsx"} onChange={singleWorkbook => setOutput({ ...output, singleWorkbook })}>{t("tasks-workbook")}</Checkbox>
      <Checkbox isSelected={output.skipEmpty} onChange={skipEmpty => setOutput({ ...output, skipEmpty })}>{t("tasks-skip-empty")}</Checkbox>
    </> },
    { id: "tables", label: t("tasks-tables"), content: <MultiOptions value={{ ...output, format: output.format === "xlsx" ? "xlsx" : "csv", allColumns: false, preserveUploadOrder: false, sitemap: defaultSitemapOptions() }}
      onChange={next => setOutput({ ...output, items: next.items, singleWorkbook: next.singleWorkbook, skipEmpty: next.skipEmpty })} onReady={ignoreCatalogReadiness} onError={reportError} /> },
    { id: "files", label: t("tasks-local-files"), content: <>{["pages", "images"].map(name => <Checkbox key={name} isSelected={task.exports.some(e => e.kind === "sitemap" && e.name === name)} onChange={checked => patch({ exports: checked ? [...task.exports, { kind: "sitemap", name, format: "csv", dir: output.dir || null }] : task.exports.filter(e => !(e.kind === "sitemap" && e.name === name)) })}>{t(`tasks-sitemap-${name}`)}</Checkbox>)}</> },
    { id: "looker", label: "Looker Studio", content: <CustomSummary value={task.customSummary ?? null} onChange={customSummary => patch({ customSummary })} /> },
    { id: "presets", label: t("tasks-presets"), content: <label className="os-field">{t("tasks-preset")}<select value={task.exportPreset ?? ""} onChange={e => {
      const preset = presets.find(p => p.name === e.target.value);
      if (preset) setOutput({ ...output, items: preset.items, singleWorkbook: preset.singleWorkbook, skipEmpty: preset.skipEmpty, preset: preset.name });
      else setOutput({ ...output, preset: null });
      setTask(v => ({ ...v, exportPreset: e.target.value || null }));
    }}><option value="">{t("tasks-none")}</option>{task.exportPreset && !presets.some(p => p.name === task.exportPreset) && <option value={task.exportPreset}>{task.exportPreset}</option>}{presets.map(p => <option key={p.name}>{p.name}</option>)}</select></label> },
    { id: "notifications", label: t("tasks-notifications"), content: notifications },
  ];
  return <Dialog title={t(task.id ? "tasks-edit" : "tasks-add")} isOpen onOpenChange={open => { if (!open && !lock.current) onClose(); }} isDismissable={!busy} size="large"
    footer={<DialogFooter okLabel={t("tasks-save")} cancelLabel={t("ui-dialog-cancel")} okDisabled={busy || !task.name.trim() || !!scheduleError} onOk={() => void save()} onCancel={() => { if (!lock.current) onClose(); }} />}>
    <fieldset className="os-tasks-editor" disabled={busy}><Tabs label={t("tasks-editor-tabs")} selected={tab} onSelect={setTab} tabs={[
      { id: "general", label: t("tasks-general"), content: general }, { id: "spider", label: "Spider", content: spider },
      { id: "apis", label: "APIs", content: <>{([["ga4", "GA4"], ["gsc", "Search Console"], ["psi", "PageSpeed Insights"], ["majestic", "Majestic"], ["ahrefs", "Ahrefs"], ["moz", "Moz"]] as const).map(([api, label]) => <Checkbox key={api} isSelected={task.apis.includes(api)} onChange={checked => patch({ apis: checked ? [...task.apis, api] : task.apis.filter(a => a !== api) })}>{label}</Checkbox>)}</> },
      { id: "exports", label: t("tasks-exports"), content: <Tabs label={t("tasks-export-tabs")} tabs={exportTabs} selected={subtab} onSelect={setSubtab} /> },
    ]} /></fieldset>
    {issues.map((issue, i) => <p key={i} role={issue.warning ? "status" : "alert"}>{issue.path}: {t(issue.messageKey, issue.args)}</p>)}
    {error && <p role="alert">{error}</p>}
  </Dialog>;
}

/** Keep wall time and its local offset, including the chosen date's DST offset. */
export function localDateWithOffset(value: string): string {
  const offset = -new Date(value).getTimezoneOffset();
  const sign = offset >= 0 ? "+" : "-";
  const hh = String(Math.floor(Math.abs(offset) / 60)).padStart(2, "0");
  const mm = String(Math.abs(offset) % 60).padStart(2, "0");
  return `${value}:00${sign}${hh}:${mm}`;
}

/** datetime-local needs wall time in the current zone, not a sliced source offset. */
export function localDateInput(value: string): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
