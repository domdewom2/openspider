// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AuthFinishLoginArgs } from "./gen/AuthFinishLoginArgs";
import type { AuthOpenLoginArgs } from "./gen/AuthOpenLoginArgs";
import type { AuthProfileInfo } from "./gen/AuthProfileInfo";
import type { AuthSaveProfileArgs } from "./gen/AuthSaveProfileArgs";
import type { NameArgs } from "./gen/NameArgs";
import type { OpHandle } from "./gen/OpHandle";
import { client } from "./client";

export const auth = {
  /** `auth.listProfiles` */
  listProfiles: (signal?: AbortSignal) => client.invoke<AuthProfileInfo[]>("auth.listProfiles", undefined, signal),
  /** `auth.saveProfile` */
  saveProfile: (args: AuthSaveProfileArgs, signal?: AbortSignal) => client.invoke<AuthProfileInfo>("auth.saveProfile", args, signal),
  /** `auth.deleteProfile` */
  deleteProfile: (args: NameArgs, signal?: AbortSignal) => client.invoke<void>("auth.deleteProfile", args, signal),
  /** `auth.openLogin` – lang laufend: liefert sofort `OpHandle`, Ergebnis (LoginCapture) über `Tick.ops[].result`. */
  openLogin: (args: AuthOpenLoginArgs, signal?: AbortSignal) => client.invoke<OpHandle>("auth.openLogin", args, signal),
  /** `auth.finishLogin` */
  finishLogin: (args: AuthFinishLoginArgs, signal?: AbortSignal) => client.invoke<AuthProfileInfo | null>("auth.finishLogin", args, signal),
} as const;
