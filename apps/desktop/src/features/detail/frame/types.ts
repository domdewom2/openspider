import type { FC } from "react";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";
import type { Selection } from "../../../ipc/gen/Selection";

export const DETAIL_TABS = ["urlDetails", "inlinks", "outlinks", "images", "resources", "serp", "rendered", "consoleLog", "source", "headers", "cookies", "duplicates", "structuredData", "lighthouse", "spelling", "a11y", "nGrams"] as const;
export type DetailTabId = typeof DETAIL_TABS[number] | "crawlPath";
export interface DetailSelection {
  /** URL ids in selection order; all top tabs publish through this same interface. */
  urlIds: number[];
  focus?: number;
  topTab?: string;
  related?: boolean;
  /** Changes when the open crawl changes, even if its URL ids are identical. */
  crawlId?: string;
}
export interface NavigationRequest { address: string; tab: "auto"; linkPath?: string; source?: boolean }
export interface DetailActions {
  navigate: (request: NavigationRequest) => void;
  back: () => void;
  openBrowser: (address: string) => void;
  export: (spec: ViewSpec, selection: Selection) => void;
  reportError: (error: unknown) => void;
}
export interface DetailPanelProps {
  selection: DetailSelection;
  actions: DetailActions;
  rendering: boolean;
  htmlStored: boolean;
  storeUncrawlable: boolean;
  positions: readonly string[];
  listMode: boolean;
}
export interface DetailPlugin { id: DetailTabId; Component: FC<DetailPanelProps> }
