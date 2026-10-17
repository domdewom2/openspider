// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ApiConnectArgs } from "./gen/ApiConnectArgs";
import type { ApiErrorRow } from "./gen/ApiErrorRow";
import type { ApiErrorsArgs } from "./gen/ApiErrorsArgs";
import type { ApiFetchArgs } from "./gen/ApiFetchArgs";
import type { ApiKindArgs } from "./gen/ApiKindArgs";
import type { ApiRow } from "./gen/ApiRow";
import type { ApiSetSettingsArgs } from "./gen/ApiSetSettingsArgs";
import type { ApiSettingsResult } from "./gen/ApiSettingsResult";
import type { ApiSourceInfo } from "./gen/ApiSourceInfo";
import type { ApiSourceSettings } from "./gen/ApiSourceSettings";
import type { ApiTestResult } from "./gen/ApiTestResult";
import type { OpHandle } from "./gen/OpHandle";
import { client } from "./client";

export const api = {
  /** `api.list` */
  list: (signal?: AbortSignal) => client.invoke<ApiSourceInfo[]>("api.list", undefined, signal),
  /** `api.connect` – lang laufend: liefert sofort `OpHandle`, Ergebnis (ApiAccount) über `Tick.ops[].result`. */
  connect: (args: ApiConnectArgs, signal?: AbortSignal) => client.invoke<OpHandle>("api.connect", args, signal),
  /** `api.disconnect` */
  disconnect: (args: ApiKindArgs, signal?: AbortSignal) => client.invoke<void>("api.disconnect", args, signal),
  /** `api.testConnection` */
  testConnection: (args: ApiKindArgs, signal?: AbortSignal) => client.invoke<ApiTestResult>("api.testConnection", args, signal),
  /** `api.getSettings` */
  getSettings: (args: ApiKindArgs, signal?: AbortSignal) => client.invoke<ApiSourceSettings>("api.getSettings", args, signal),
  /** `api.setSettings` */
  setSettings: (args: ApiSetSettingsArgs, signal?: AbortSignal) => client.invoke<ApiSettingsResult>("api.setSettings", args, signal),
  /** `api.status` */
  status: (signal?: AbortSignal) => client.invoke<ApiRow[]>("api.status", undefined, signal),
  /** `api.fetch` – lang laufend: liefert sofort `OpHandle`, Ergebnis (ApiFetchSummary) über `Tick.ops[].result`. */
  fetch: (args: ApiFetchArgs, signal?: AbortSignal) => client.invoke<OpHandle>("api.fetch", args, signal),
  /** `api.errors` */
  errors: (args: ApiErrorsArgs, signal?: AbortSignal) => client.invoke<ApiErrorRow[]>("api.errors", args, signal),
} as const;
