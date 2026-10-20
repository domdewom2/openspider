import { create } from "zustand";
import { currentLanguage, i18n, getT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import type { TaskDef } from "../../ipc/gen/TaskDef";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import type { TaskRunRow } from "../../ipc/gen/TaskRunRow";

export const useTaskWindow = create<{ open: boolean; setOpen(open: boolean): void }>(set => ({ open: false, setOpen: open => set({ open }) }));
export const openTasks = () => useTaskWindow.getState().setOpen(true);

// Handles live outside the dialog so closing/reopening never loses the abort action.
export const useTaskOperations = create<{ handles: Record<string, OpHandle>; track(key: string, handle: OpHandle): void }>(set => ({
  handles: {}, track: (key, handle) => set(s => ({ handles: { ...s.handles, [key]: handle } })),
}));

export function newTask(): TaskDef {
  return { id: null, name: "", project: null, enabled: true, schedule: { kind: "daily", time: "03:00" },
    crawl: { source: { kind: "spider", url: "" }, profile: null, authProfile: null, overrides: null },
    execution: { headless: true, runIfMissed: true, autoCompare: false }, apis: [], exports: [],
    notify: { onSuccess: false, onFailure: true, account: null, to: [] } };
}

export function taskError(error: unknown): string {
  const e = toCmdError(error);
  const text = describeError(getT(), e, key => i18n.has(currentLanguage(), key)).text;
  // SMTP codes are part of the public error args, never the potentially sensitive detail.
  return e.args.smtpCode ? `${text} (SMTP ${e.args.smtpCode})` : text;
}

export function runResult(row: TaskRunRow): string {
  const key = row.state === "succeeded" && row.exitCode === 4 ? "warnings" : row.state;
  return getT()(`tasks-result-${key}`);
}

export function duration(row: TaskRunRow): string {
  if (!row.endedAt) return "—";
  return `${Math.max(0, Math.round((Date.parse(row.endedAt) - Date.parse(row.startedAt)) / 1000))} s`;
}
