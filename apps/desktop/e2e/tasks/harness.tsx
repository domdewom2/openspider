import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client, createClient, setClient } from "../../src/ipc/client";
import { taskFixture } from "../../src/features/tasks/fixtures/core";
import fixture from "../../src/features/tasks/fixtures/out-schedule.json";
import type { TaskRunRow } from "../../src/ipc/gen/TaskRunRow";
import type { TaskDef } from "../../src/ipc/gen/TaskDef";
import { openTasks, TasksHost } from "../../src/features/tasks";
import { EmailSettings } from "../../src/features/settings/sections/email";
import { useSession } from "../../src/state/session";
import { useTaskOperations } from "../../src/features/tasks/model";
import "../../src/ui/ui.css";

const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage("en");
const fx = runtime.transportName === "mock" ? taskFixture() : null;
if (fx) setClient(createClient(fx.core.transport("main")));
const api = {
  calls: () => fx?.core.calls ?? [],
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
  scheduledRun() { if (fx) fx.control.lastRun = structuredClone(fixture.runningRun) as TaskRunRow; },
  prepareOnceImport() { if (fx) fx.control.importedTask = structuredClone(fixture.importedOnce) as TaskDef; },
  complete() {
    const handles = Object.values(useTaskOperations.getState().handles);
    useSession.setState({ ops: Object.fromEntries(handles.map(handle => [handle.opId, { opId: handle.opId, labelKey: "tasks-op-done", done: 1, state: "done" as const, result: [] }])) });
  },
};
declare global { interface Window { taskFixture: typeof api } }
window.taskFixture = api;
createRoot(document.getElementById("root")!).render(<main>
  <button onClick={openTasks}>Scheduling</button>
  <TasksHost desktop={{ choose: async kind => kind === "export" ? "fixture.ostask" : "import.ostask" }} />
  <h1>Email settings</h1><EmailSettings />
</main>);
