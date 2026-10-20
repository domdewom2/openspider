import type { ComponentType } from "react";

/** Optional main-window regions; absent registrations retain the scaffold defaults. */
export const mainRegions: { tabs?: ComponentType; table?: ComponentType; detail?: ComponentType } = {};

export function registerMainRegions(regions: Partial<typeof mainRegions>): void {
  Object.assign(mainRegions, regions);
}
