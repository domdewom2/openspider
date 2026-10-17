// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ProjectCreateArgs } from "./gen/ProjectCreateArgs";
import type { ProjectInfo } from "./gen/ProjectInfo";
import type { ProjectLockArgs } from "./gen/ProjectLockArgs";
import type { ProjectRenameArgs } from "./gen/ProjectRenameArgs";
import { client } from "./client";

export const projects = {
  /** `projects.create` */
  create: (args: ProjectCreateArgs, signal?: AbortSignal) => client.invoke<ProjectInfo>("projects.create", args, signal),
  /** `projects.rename` */
  rename: (args: ProjectRenameArgs, signal?: AbortSignal) => client.invoke<void>("projects.rename", args, signal),
  /** `projects.lock` */
  lock: (args: ProjectLockArgs, signal?: AbortSignal) => client.invoke<void>("projects.lock", args, signal),
} as const;
