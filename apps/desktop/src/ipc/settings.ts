// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AppSettingsDto } from "./gen/AppSettingsDto";
import type { SettingsPatch } from "./gen/SettingsPatch";
import { client } from "./client";

export const settings = {
  /** `settings.get` */
  get: (signal?: AbortSignal) => client.invoke<AppSettingsDto>("settings.get", undefined, signal),
  /** `settings.set` */
  set: (args: SettingsPatch, signal?: AbortSignal) => client.invoke<AppSettingsDto>("settings.set", args, signal),
} as const;
