// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AppSettingsDto } from "./gen/AppSettingsDto";
import type { CertificateChainDto } from "./gen/CertificateChainDto";
import type { CertificateProbeArgs } from "./gen/CertificateProbeArgs";
import type { ProxyPasswordArgs } from "./gen/ProxyPasswordArgs";
import type { SettingsPatch } from "./gen/SettingsPatch";
import { client } from "./client";

export const settings = {
  /** `settings.probeCertificates` */
  probeCertificates: (args: CertificateProbeArgs, signal?: AbortSignal) => client.invoke<CertificateChainDto>("settings.probeCertificates", args, signal),
  /** `settings.setProxyPassword` */
  setProxyPassword: (args: ProxyPasswordArgs, signal?: AbortSignal) => client.invoke<void>("settings.setProxyPassword", args, signal),
  /** `settings.get` */
  get: (signal?: AbortSignal) => client.invoke<AppSettingsDto>("settings.get", undefined, signal),
  /** `settings.set` */
  set: (args: SettingsPatch, signal?: AbortSignal) => client.invoke<AppSettingsDto>("settings.set", args, signal),
} as const;
