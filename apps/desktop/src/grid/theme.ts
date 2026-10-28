import { SIZES } from "../ui/tokens/tokens";

/** K2-77: same physical head heights as the CSS/data-view density. */
export const gridHeaderHeight = (rowHeight: number) => rowHeight === SIZES.rowHeight.normal ? SIZES.headerHeight.normal : rowHeight + SIZES.headerHeight.compact - SIZES.rowHeight.compact;

import type { TokenMap } from "../ui/tokens/tokens";
/** Text remains present: status is never conveyed by colour alone (UI-126). */
export function cellBadge(column: string, text: string, tokens: TokenMap) {
  if (!text || text === "…") return null;
  let kind: "success" | "warning" | "danger" | "neutral";
  if (column === "Status Code") {
    const code = Number(text);
    kind = code >= 500 ? "danger" : code >= 400 ? "danger" : code >= 300 ? "warning" : code >= 200 ? "success" : "neutral";
  } else if (column === "Indexability") kind = /^(indexable|indexierbar)$/i.test(text) ? "success" : "warning";
  else if (column === "Status") kind = /^(ok|success)$/i.test(text) ? "success" : /error|failed|not found|fehler/i.test(text) ? "danger" : "neutral";
  else return null;
  return { text: tokens[`badge.${kind}`], background: tokens[`badge.${kind}Bg`] };
}
