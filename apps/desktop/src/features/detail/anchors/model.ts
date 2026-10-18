import type { ViewSpec } from "../../../ipc/gen/ViewSpec";
import type { SortKey } from "../../../ipc/gen/SortKey";
export interface LinkOptions { id: string; position: string; excludeSelf: boolean; excludeNofollow: boolean; mode: "links" | "anchors"; anchor?: string }
export const DEFAULT_LINK_OPTIONS: LinkOptions = { id: "All", position: "", excludeSelf: false, excludeNofollow: false, mode: "links" };
export function linkSpec(direction: "inlinks" | "outlinks", urlIds: number[], options: LinkOptions, search: string, sort: SortKey[]): ViewSpec {
  return { kind: direction, urlIds, filter: { id: options.id, position: options.position || null, excludeSelf: options.excludeSelf, excludeNofollow: options.excludeNofollow, mode: options.mode, anchor: options.anchor ?? null }, search, sort: sort.length ? sort : options.mode === "anchors" ? [{ column: "Links", dir: "desc" }, { column: "Anchor Text", dir: "asc" }] : [] };
}
export const LINK_COLUMNS = ["Type", "From", "To", "Anchor Text", "Alt Text", "Follow", "Target", "Rel", "Status Code", "Status", "Path Type", "Link Path", "Link Position", "Link Origin", "Original URL"];
export function linkColumns(direction: "inlinks" | "outlinks", mode: "links" | "anchors"): string[] {
  return mode === "anchors" ? ["Anchor Text", "Links", direction === "inlinks" ? "Unique Sources" : "Unique Targets", "% of Links"] : [...LINK_COLUMNS, ...(direction === "outlinks" ? ["Link Crawlability"] : [])];
}
