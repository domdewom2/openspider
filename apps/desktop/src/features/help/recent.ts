import { updateUiState } from "../../state/uistate";
// CRAWL-127: exakt zehn verschiedene Pfade, zuletzt geöffnet zuerst. Keine Pfade im Frontend konstruieren.
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import { crawls } from "../../ipc/crawls";
import { uistate } from "../../ipc/uistate";
import type { RecentEntry } from "../../ui/menu/contributions/main";

export function normalizeRecent(value: unknown): RecentEntry[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  return value.filter((entry): entry is RecentEntry => {
    if (!entry || typeof entry.path !== "string" || !entry.path || seen.has(entry.path)) return false;
    seen.add(entry.path);
    return true;
  }).slice(0, 10).map(({ path, label }) => ({ path, ...(typeof label === "string" && label ? { label } : {}) }));
}
function record(value: unknown): Record<string, JsonValue> {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, JsonValue> : {};
}
export async function loadRecent(): Promise<RecentEntry[]> {
  const saved = await uistate.load();
  const entries = normalizeRecent(record(saved.state).recentCrawls);
  if (!entries.length) return entries;
  // The index supplies metadata without opening/migrating every recent crawl.
  try {
    const index = await crawls.list({ show: "all" });
    return entries.map(entry => {
      const row = index.find(row => row.crawl.file === entry.path);
      return row ? { path: entry.path, label: `${row.startUrl || row.crawl.name} · ${row.crawl.startedAt.slice(0, 10)}` } : entry;
    });
  } catch { return entries; }
}
/** Replay the list operation, rather than a snapshot, after revision conflicts. */
export async function updateRecent(change: (entries: RecentEntry[]) => RecentEntry[]): Promise<void> {
  await updateUiState(state => ({ ...state, recentCrawls: normalizeRecent(change(normalizeRecent(state.recentCrawls))).map(({ path, label }) => ({ path, ...(label ? { label } : {}) })) }));
}
