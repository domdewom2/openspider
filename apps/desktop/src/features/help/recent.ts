import { updateUiState } from "../../state/uistate";
// CRAWL-127: exakt zehn verschiedene Pfade, zuletzt geöffnet zuerst. Keine Pfade im Frontend konstruieren.
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import { uistate } from "../../ipc/uistate";
import type { RecentEntry } from "../../ui/menu/contributions/main";

export function normalizeRecent(value: unknown): RecentEntry[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  return value.filter((entry): entry is RecentEntry => {
    if (!entry || typeof entry.path !== "string" || !entry.path || seen.has(entry.path)) return false;
    seen.add(entry.path);
    return true;
  }).slice(0, 10).map(({ path }) => ({ path }));
}
function record(value: unknown): Record<string, JsonValue> {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, JsonValue> : {};
}
export async function loadRecent(): Promise<RecentEntry[]> {
  const saved = await uistate.load();
  return normalizeRecent(record(saved.state).recentCrawls);
}
/** Replay the list operation, rather than a snapshot, after revision conflicts. */
export async function updateRecent(change: (entries: RecentEntry[]) => RecentEntry[]): Promise<void> {
  await updateUiState(state => ({ ...state, recentCrawls: normalizeRecent(change(normalizeRecent(state.recentCrawls))).map(({ path }) => ({ path })) }));
}
