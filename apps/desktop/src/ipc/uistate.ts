// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { UiStateJson } from "./gen/UiStateJson";
import { client } from "./client";

export const uistate = {
  /** `uistate.load` */
  load: (signal?: AbortSignal) => client.invoke<UiStateJson>("uistate.load", undefined, signal),
  /** `uistate.save` */
  save: (args: UiStateJson, signal?: AbortSignal) => client.invoke<UiStateJson>("uistate.save", args, signal),
} as const;
