// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ColumnMeta } from "./gen/ColumnMeta";
import type { ColumnsArgs } from "./gen/ColumnsArgs";
import type { FilterCounts } from "./gen/FilterCounts";
import type { ScopeArgs } from "./gen/ScopeArgs";
import type { TabInfo } from "./gen/TabInfo";
import { client } from "./client";

export const catalog = {
  /** `catalog.tabs` */
  tabs: (signal?: AbortSignal) => client.invoke<TabInfo[]>("catalog.tabs", undefined, signal),
  /** `catalog.columns` */
  columns: (args: ColumnsArgs, signal?: AbortSignal) => client.invoke<ColumnMeta[]>("catalog.columns", args, signal),
  /** `catalog.filterCounts` */
  filterCounts: (args: ScopeArgs, signal?: AbortSignal) => client.invoke<FilterCounts>("catalog.filterCounts", args, signal),
} as const;
