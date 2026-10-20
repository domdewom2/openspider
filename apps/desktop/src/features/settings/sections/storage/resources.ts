import { z } from "zod";
import { settings } from "../../../../ipc/settings";
import { rememberPendingRestarts } from "./restart";

const schema = z.object({
  physicalBytes: z.number().positive(), usedBytes: z.number().nonnegative(), ramLimitBytes: z.number().positive(),
  cacheLimitMb: z.number().positive(), renderProcessLimit: z.number().int().positive(),
  restartRequired: z.array(z.string()).default([]),
});
export type Resources = z.infer<typeof schema>;
export const GiB = 2 ** 30;
export const ramMaximum = (physicalBytes: number) => Math.max(1, Math.floor((physicalBytes / GiB - 2) * 10) / 10);
export async function readResources(): Promise<Resources> {
  const snapshot = schema.parse(await settings.resources());
  rememberPendingRestarts(snapshot.restartRequired);
  return snapshot;
}
