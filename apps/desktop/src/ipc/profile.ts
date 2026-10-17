// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ConfigProfileInfo } from "./gen/ConfigProfileInfo";
import type { CrawlConfigDto } from "./gen/CrawlConfigDto";
import type { NameArgs } from "./gen/NameArgs";
import type { PathArgs } from "./gen/PathArgs";
import type { ProfileExportArgs } from "./gen/ProfileExportArgs";
import type { ProfileSaveArgs } from "./gen/ProfileSaveArgs";
import type { SetDefaultProfileArgs } from "./gen/SetDefaultProfileArgs";
import { client } from "./client";

export const profile = {
  /** `profile.list` */
  list: (signal?: AbortSignal) => client.invoke<ConfigProfileInfo[]>("profile.list", undefined, signal),
  /** `profile.load` */
  load: (args: NameArgs, signal?: AbortSignal) => client.invoke<CrawlConfigDto>("profile.load", args, signal),
  /** `profile.save` */
  save: (args: ProfileSaveArgs, signal?: AbortSignal) => client.invoke<ConfigProfileInfo>("profile.save", args, signal),
  /** `profile.delete` */
  delete: (args: NameArgs, signal?: AbortSignal) => client.invoke<void>("profile.delete", args, signal),
  /** `profile.setDefault` */
  setDefault: (args: SetDefaultProfileArgs, signal?: AbortSignal) => client.invoke<void>("profile.setDefault", args, signal),
  /** `profile.import` */
  import: (args: PathArgs, signal?: AbortSignal) => client.invoke<ConfigProfileInfo>("profile.import", args, signal),
  /** `profile.export` */
  export: (args: ProfileExportArgs, signal?: AbortSignal) => client.invoke<void>("profile.export", args, signal),
} as const;
