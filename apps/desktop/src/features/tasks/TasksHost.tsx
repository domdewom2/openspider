import { useCallback, useEffect, useRef, useState } from "react";
import { useT, useFormat } from "../../i18n";
import { tasks } from "../../ipc/tasks";
import { op } from "../../ipc/op";
import { diag } from "../../ipc/diag";
import type { TaskDef } from "../../ipc/gen/TaskDef";
import type { TaskInfo } from "../../ipc/gen/TaskInfo";
import type { TaskRunRow } from "../../ipc/gen/TaskRunRow";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import { useSession } from "../../state/session";
import { useAction } from "../../ui/actions";
import { registerMenuContributions } from "../../ui/menu/registry";
import { Button, Dialog, DialogFooter } from "../../ui/primitives";
import { TaskEditor } from "./TaskEditor";
import { duration, newTask, openTasks, runResult, taskError, useTaskOperations, useTaskWindow } from "./model";
import { taskDesktop, type TaskDesktop } from "./native";
import "./tasks.css";

interface Row { info: TaskInfo; task?: TaskDef; reasons: string[] }

export function TasksHost({ desktop = taskDesktop }: { desktop?: TaskDesktop }) {
  const open = useTaskWindow(s => s.open);
  useAction("tasks.open", openTasks);
  useEffect(() => registerMenuContributions("features/tasks", [{ menu: "file", group: "tasks", order: 40, items: [{ id: "tasks.open", action: "tasks.open", labelKey: "tasks-title" }] }]), []);
  // Operation handles are kept in the feature store when this dialog is closed.
  return open ? <TaskManager desktop={desktop} onClose={() => useTaskWindow.getState().setOpen(false)} /> : null;
}

export function TaskManager({ desktop = taskDesktop, onClose }: { desktop?: TaskDesktop; onClose(): void }) {
  const t = useT();
  const f = useFormat();
  const [rows, setRows] = useState<Row[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [editor, setEditor] = useState<TaskDef | null>(null);
  const [history, setHistory] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const lock = useRef(false);
  const request = useRef(0);
  const seen = useRef(new Set<string>());
  const handles = useTaskOperations(s => s.handles);
  const statuses = useSession(s => s.ops);
  const refresh = useCallback(async () => {
    const sequence = ++request.current;
    const list = await tasks.list();
    const result = await Promise.all(list.map(async info => {
      try {
        const task = await tasks.get({ id: info.id });
        const issues = await tasks.validate({ task });
        return { info, task, reasons: [...issues.filter(i => !i.warning).map(i => `${i.path}: ${t(i.messageKey, i.args)}`), ...(info.enabled && !info.registered ? [t("tasks-not-registered")] : [])] };
      } catch (e) { return { info, reasons: [taskError(e)] }; }
    }));
    if (sequence !== request.current) return;
    setRows(result); setLoaded(true);
    setSelected(ids => ids.filter(id => result.some(r => r.info.id === id)));
  }, [t]);
  useEffect(() => {
    void refresh().catch(e => setError(taskError(e)));
    return () => { request.current++; };
  }, [refresh]);
  useEffect(() => {
    let changed = false;
    for (const handle of Object.values(handles)) {
      const status = statuses[handle.opId];
      if (!status || status.state === "running" || seen.current.has(handle.opId)) continue;
      seen.current.add(handle.opId); changed = true;
      if (status.state === "failed") setError(status.error ? taskError(status.error) : t("tasks-operation-failed"));
    }
    if (changed) void refresh().catch(e => setError(taskError(e)));
  }, [handles, statuses, refresh, t]);
  const act = async (fn: () => Promise<void>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try { await fn(); } catch (e) { setError(taskError(e)); }
    finally { lock.current = false; setBusy(false); }
  };
  const row = rows.find(r => r.info.id === selected[0]);
  const one = selected.length === 1 ? row : undefined;
  const track = (key: string, handle: OpHandle) => useTaskOperations.getState().track(key, handle);
  const running = (handle: OpHandle) => !statuses[handle.opId] || statuses[handle.opId].state === "running";
  const transferBusy = Object.entries(handles).some(([key, handle]) => key.startsWith("transfer:") && running(handle));
  const date = (value?: string | null) => value ? f.dateTime(value) : "—";
  return <>
    <Dialog title={t("tasks-title")} isOpen onOpenChange={value => { if (!value && !busy) onClose(); }} size="large" isDismissable={!busy}
      footer={<Button onPress={onClose} isDisabled={busy}>{t("ui-dialog-close")}</Button>}>
      <div className="os-tasks-actions" aria-label={t("tasks-actions")}>
        <Button isDisabled={busy} onPress={() => setEditor(newTask())}>{t("tasks-add")}</Button>
        <Button isDisabled={busy || !one?.task} onPress={() => { if (one?.task) setEditor(one.task); }}>{t("tasks-edit")}</Button>
        <Button isDisabled={busy || !one?.task} onPress={() => { if (one?.task) setEditor({ ...structuredClone(one.task), id: null, name: `${one.task.name} ${t("tasks-copy")}` }); }}>{t("tasks-duplicate")}</Button>
        <Button isDisabled={busy || !one} onPress={() => { if (one) setDeleting(one.info.id); }}>{t("tasks-delete")}</Button>
        <Button isDisabled={busy || !one || !!one.reasons.length} onPress={() => void act(async () => { if (one) track(`run:${one.info.id}:${Date.now()}`, await tasks.run({ id: one.info.id })); })}>{t("tasks-run")}</Button>
        <Button isDisabled={busy || !one} onPress={() => { if (one) setHistory(one.info.id); }}>{t("tasks-history")}</Button>
        <Button isDisabled={busy || transferBusy || !selected.length} onPress={() => void act(async () => { const path = await desktop.choose("export"); if (path) track("transfer:export", await tasks.export({ ids: selected, path })); })}>{t("tasks-export")}</Button>
        <Button isDisabled={busy || transferBusy} onPress={() => void act(async () => { const path = await desktop.choose("import"); if (path) track("transfer:import", await tasks.import({ path })); })}>{t("tasks-import")}</Button>
        <Button isDisabled={busy} onPress={() => void act(refresh)}>{t("tasks-refresh")}</Button>
      </div>
      {error && <p role="alert">{error}</p>}
      {!loaded && !error && <p role="status">{t("tasks-loading")}</p>}
      {loaded && !rows.length && <p>{t("tasks-empty")}</p>}
      <div className="os-tasks-scroll"><table className="os-tasks-table" aria-label={t("tasks-title")}><thead><tr>
        {["select", "name", "project", "next-run", "interval", "mode", "source", "status", "last-result"].map(key => <th key={key} scope="col">{t(`tasks-${key}`)}</th>)}
      </tr></thead><tbody>{rows.map(({ info, task, reasons }) => <tr key={info.id}>
        <td><input type="checkbox" aria-label={t("tasks-select-name", { name: info.name })} checked={selected.includes(info.id)} onChange={e => setSelected(ids => e.target.checked ? [...ids, info.id] : ids.filter(id => id !== info.id))} /></td>
        <td>{info.name}</td><td>{info.project}</td><td>{date(info.nextRun)}</td><td>{t(`tasks-${info.schedule.kind}`)} {"time" in info.schedule ? info.schedule.time : date(info.schedule.at)}</td>
        <td>{task?.crawl.source.kind === "spider" ? "Spider" : task ? t("tasks-list-mode") : "—"}</td><td>{task?.crawl.source.kind === "spider" ? task.crawl.source.url : task?.crawl.source.path}</td>
        <td>{reasons.length ? t("tasks-invalid") : t("tasks-valid")}{reasons.map((reason, i) => <p key={i}>{reason}</p>)}{!info.enabled && <small>{t("tasks-disabled")}</small>}</td>
        <td>{info.lastRun ? runResult(info.lastRun) : "—"}</td>
      </tr>)}</tbody></table></div>
      <div aria-live="polite">{Object.entries(handles).map(([key, handle]) => {
        const status = statuses[handle.opId];
        return <div key={handle.opId} className="os-tasks-actions"><span>{key.startsWith("run:") ? rows.find(r => key.startsWith(`run:${r.info.id}:`))?.info.name : t("tasks-transfer")} — {status ? t(`tasks-op-${status.state}`) : t("tasks-op-running")}</span>
          {running(handle) && handle.cancellable && <Button isDisabled={busy} onPress={() => void act(() => op.cancel({ opId: handle.opId }))}>{t("tasks-abort")}</Button>}
        </div>;
      })}</div>
    </Dialog>
    {editor && <TaskEditor initial={editor} onClose={() => setEditor(null)} onSaved={() => { setEditor(null); void act(refresh); }} />}
    {history && <TaskHistory id={history} onClose={() => setHistory(null)} />}
    {deleting && <Dialog title={t("tasks-delete")} description={t("tasks-delete-confirm")} isOpen onOpenChange={open => { if (!open && !busy) setDeleting(null); }}
      footer={<DialogFooter okLabel={t("tasks-delete")} cancelLabel={t("ui-dialog-cancel")} okVariant="danger" okDisabled={busy} onCancel={() => setDeleting(null)} onOk={() => void act(async () => { await tasks.delete({ id: deleting }); setDeleting(null); await refresh(); })} />}>{error && <p role="alert">{error}</p>}</Dialog>}
  </>;
}

export function TaskHistory({ id, onClose }: { id: string; onClose(): void }) {
  const t = useT();
  const f = useFormat();
  const [rows, setRows] = useState<TaskRunRow[]>([]);
  const [offset, setOffset] = useState(0);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const abort = new AbortController(); setBusy(true); setError(null);
    void tasks.history({ id, offset, limit: 100 }, abort.signal).then(setRows).catch(e => { if (!abort.signal.aborted) setError(taskError(e)); }).finally(() => { if (!abort.signal.aborted) setBusy(false); });
    return () => abort.abort();
  }, [id, offset]);
  const date = (value?: string | null) => value ? f.dateTime(value) : "—";
  return <Dialog title={t("tasks-history")} isOpen onOpenChange={open => { if (!open) onClose(); }} size="large" footer={<Button onPress={onClose}>{t("ui-dialog-close")}</Button>}>
    {error && <p role="alert">{error}</p>}
    <div className="os-tasks-scroll"><table className="os-tasks-table" aria-busy={busy}><thead><tr>{["started", "finished", "duration", "result", "crawl-id", "output-files", "message"].map(key => <th key={key} scope="col">{t(`tasks-${key}`)}</th>)}</tr></thead>
      <tbody>{rows.map(run => <tr key={run.runId}><td>{date(run.startedAt)}</td><td>{date(run.endedAt)}</td><td>{duration(run)}</td><td>{runResult(run)}</td><td>{run.crawlId ?? "—"}</td><td>{run.exports.map(path => <div key={path}>{path}</div>)}</td><td>{run.error}</td></tr>)}</tbody></table></div>
    <div className="os-tasks-actions"><Button isDisabled={busy || offset === 0} onPress={() => setOffset(n => Math.max(0, n - 100))}>{t("tasks-previous")}</Button><Button isDisabled={busy || rows.length < 100} onPress={() => setOffset(n => n + 100)}>{t("tasks-next")}</Button>
      <Button onPress={() => void diag.openLogs().catch(e => setError(taskError(e)))}>{t("tasks-open-logs")}</Button></div>
  </Dialog>;
}
