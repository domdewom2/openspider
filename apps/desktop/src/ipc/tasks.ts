// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { IdArgs } from "./gen/IdArgs";
import type { OpHandle } from "./gen/OpHandle";
import type { PathArgs } from "./gen/PathArgs";
import type { PlannerPreview } from "./gen/PlannerPreview";
import type { TaskArgs } from "./gen/TaskArgs";
import type { TaskDef } from "./gen/TaskDef";
import type { TaskInfo } from "./gen/TaskInfo";
import type { TaskRegisterArgs } from "./gen/TaskRegisterArgs";
import type { TaskRunRow } from "./gen/TaskRunRow";
import type { TasksExportArgs } from "./gen/TasksExportArgs";
import type { TasksHistoryArgs } from "./gen/TasksHistoryArgs";
import type { ValidationIssue } from "./gen/ValidationIssue";
import { client } from "./client";

export const tasks = {
  /** `tasks.list` */
  list: (signal?: AbortSignal) => client.invoke<TaskInfo[]>("tasks.list", undefined, signal),
  /** `tasks.get` */
  get: (args: IdArgs, signal?: AbortSignal) => client.invoke<TaskDef>("tasks.get", args, signal),
  /** `tasks.validate` */
  validate: (args: TaskArgs, signal?: AbortSignal) => client.invoke<ValidationIssue[]>("tasks.validate", args, signal),
  /** `tasks.save` */
  save: (args: TaskArgs, signal?: AbortSignal) => client.invoke<TaskInfo>("tasks.save", args, signal),
  /** `tasks.delete` */
  delete: (args: IdArgs, signal?: AbortSignal) => client.invoke<void>("tasks.delete", args, signal),
  /** `tasks.run` – lang laufend: liefert sofort `OpHandle`, Ergebnis (TaskRunRow) über `Tick.ops[].result`. */
  run: (args: IdArgs, signal?: AbortSignal) => client.invoke<OpHandle>("tasks.run", args, signal),
  /** `tasks.history` */
  history: (args: TasksHistoryArgs, signal?: AbortSignal) => client.invoke<TaskRunRow[]>("tasks.history", args, signal),
  /** `tasks.register` */
  register: (args: TaskRegisterArgs, signal?: AbortSignal) => client.invoke<PlannerPreview>("tasks.register", args, signal),
  /** `tasks.unregister` */
  unregister: (args: IdArgs, signal?: AbortSignal) => client.invoke<void>("tasks.unregister", args, signal),
  /** `tasks.export` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  export: (args: TasksExportArgs, signal?: AbortSignal) => client.invoke<OpHandle>("tasks.export", args, signal),
  /** `tasks.import` – lang laufend: liefert sofort `OpHandle`, Ergebnis (TaskInfo[]) über `Tick.ops[].result`. */
  import: (args: PathArgs, signal?: AbortSignal) => client.invoke<OpHandle>("tasks.import", args, signal),
} as const;
