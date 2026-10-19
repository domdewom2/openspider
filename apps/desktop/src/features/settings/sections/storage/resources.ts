import { z } from "zod";
import { settings } from "../../../../ipc/settings";

const schema = z.object({
  physicalBytes: z.number().positive(), usedBytes: z.number().nonnegative(), ramLimitBytes: z.number().positive(),
  cacheLimitMb: z.number().positive(), renderProcessLimit: z.number().int().positive(),
});
export type Resources = z.infer<typeof schema>;
export const GiB = 2 ** 30;
export const ramMaximum = (physicalBytes: number) => Math.max(1, Math.floor((physicalBytes / GiB - 2) * 10) / 10);
export async function readResources(): Promise<Resources> { return schema.parse(await settings.resources()); }
