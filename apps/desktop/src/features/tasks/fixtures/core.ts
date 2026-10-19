// Contract fixture shared by Vitest and browser tests; never imported by production modules.
import { MockCore } from "../../../ipc/transport/mock";
import type { TaskDef } from "../../../ipc/gen/TaskDef";
import type { TaskInfo } from "../../../ipc/gen/TaskInfo";
import type { MailAccountInfo } from "../../../ipc/gen/MailAccountInfo";
import type { MailAccountInput } from "../../../ipc/gen/MailAccountInput";
import type { ValidationIssue } from "../../../ipc/gen/ValidationIssue";
import fixture from "./out-schedule.json";

export function taskFixture() {
  const core = new MockCore();
  let definitions: TaskDef[] = [structuredClone(fixture.task) as TaskDef];
  let accounts: MailAccountInfo[] = [structuredClone(fixture.account) as MailAccountInfo];
  const registered = new Set(["nightly"]);
  const control = { issues: [] as ValidationIssue[], registrationsFail: false, nextId: 0, operation: 0 };
  const info = (task: TaskDef): TaskInfo => ({ ...fixture.info, id: task.id!, name: task.name, project: task.project, schedule: task.schedule, enabled: task.enabled, registered: registered.has(task.id!) });
  core.handlers.set("tasks.list", () => definitions.map(info));
  core.handlers.set("tasks.get", args => structuredClone(definitions.find(d => d.id === (args as { id: string }).id)!));
  core.handlers.set("tasks.validate", () => control.issues);
  core.handlers.set("tasks.save", args => {
    const task = structuredClone((args as { task: TaskDef }).task);
    task.id ??= `task-${++control.nextId}`;
    definitions = [...definitions.filter(d => d.id !== task.id), task];
    return info(task);
  });
  core.handlers.set("tasks.register", args => {
    if (control.registrationsFail) throw new Error("Fixture planner unavailable");
    registered.add((args as { id: string }).id);
    return { backend: "launchd", files: [], command: [] };
  });
  core.handlers.set("tasks.unregister", args => { registered.delete((args as { id: string }).id); });
  core.handlers.set("tasks.delete", args => { const id = (args as { id: string }).id; definitions = definitions.filter(d => d.id !== id); registered.delete(id); });
  core.handlers.set("tasks.history", () => fixture.runs);
  for (const command of ["tasks.run", "tasks.export", "tasks.import", "mail.testAccount"]) core.handlers.set(command, () => ({ opId: `operation-${++control.operation}`, labelKey: "tasks-op-running", cancellable: true }));
  core.handlers.set("op.cancel", () => undefined);
  core.handlers.set("mail.listAccounts", () => accounts);
  core.handlers.set("mail.saveAccount", args => {
    const account = (args as { account: MailAccountInput }).account;
    const previous = accounts.find(a => a.id === account.id);
    const saved: MailAccountInfo = { id: account.id ?? "new-mail", label: account.label, host: account.host, port: account.port, tls: account.tls, from: account.from, hasSecret: !!account.password || !!previous?.hasSecret };
    accounts = [...accounts.filter(a => a.id !== saved.id), saved];
    return saved;
  });
  core.handlers.set("mail.deleteAccount", args => { accounts = accounts.filter(a => a.id !== (args as { id: string }).id); });
  core.handlers.set("export.customSummaryMetrics", () => fixture.metrics);
  core.handlers.set("export.catalog", () => ({ bulk: [], reports: [], sitemaps: [] }));
  core.handlers.set("export.presetList", () => []);
  core.handlers.set("diag.openLogs", () => undefined);
  return { core, control, definitions: () => definitions };
}
