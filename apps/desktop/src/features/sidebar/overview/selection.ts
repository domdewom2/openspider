import { create } from "zustand";

/** Canonical summary metrics of UI-061, in display order (shared by Overview, chart and dashboard). */
export const SUMMARY_KEYS = ["Total URLs Encountered", "Total URLs Crawled", "Total Internal Blocked by robots.txt", "Total External Blocked by robots.txt", "Total URLs Displayed", "Total Internal URLs", "Total External URLs", "Total Internal Indexable URLs", "Total Internal Non-Indexable URLs"];

/** A selectable Overview entry (UI-102): the Summary block, a tab header or a filter row. */
export type EntryRef = { kind: "summary" } | { kind: "tab"; tab: string } | { kind: "filter"; tab: string; filter: string };

export function entryKey(entry: EntryRef): string {
  return entry.kind === "summary" ? "summary" : entry.kind === "tab" ? `tab:${entry.tab}` : `filter:${entry.tab}\u001f${entry.filter}`;
}

/** The chart of a filter row is the chart of its tab block. */
export function chartEntry(entry: EntryRef): EntryRef {
  return entry.kind === "filter" ? { kind: "tab", tab: entry.tab } : entry;
}

/**
 * UI-102 selection: kept across live updates, sidebar tab changes and collapsing the block; dropped when another crawl
 * is opened or the crawl is cleared (the crawl id changes), never persisted across restarts.
 */
export const useOverviewSelection = create<{
  crawl: string | null;
  entry: EntryRef | null;
  select(entry: EntryRef | null): void;
  forCrawl(crawl: string | null): void;
}>((set, get) => ({
  crawl: null,
  entry: null,
  select(entry) {
    const current = get().entry;
    if (current && entry && entryKey(current) === entryKey(entry)) return;
    set({ entry });
  },
  forCrawl(crawl) {
    if (get().crawl !== crawl) set({ crawl, entry: null });
  },
}));

/** Focuses the tree item of the current selection (Esc from the chart, UI-102). */
export function focusSelectedEntry(root: ParentNode = document): boolean {
  const entry = useOverviewSelection.getState().entry;
  if (!entry) return false;
  const keys = entry.kind === "filter" ? [entryKey(entry), entryKey({ kind: "tab", tab: entry.tab })] : [entryKey(entry)];
  const items = [...root.querySelectorAll<HTMLElement>("[data-ovc-entry]")];
  for (const key of keys) {
    const el = items.find(item => item.dataset.ovcEntry === key);
    if (el) { el.focus(); return true; }
  }
  return false;
}
