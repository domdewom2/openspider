import { IpcError, toCmdError } from "../../ipc/errors";
import { client } from "../../ipc/client";
import type { ViewDataSource } from "../../data/ViewDataSource";
import { SelectionModel } from "../../state/selection";
import { view } from "../../ipc/view";
import type { Selection } from "../../ipc/gen/Selection";

export function contextSelection(selection: SelectionModel, id: number): SelectionModel {
  return selection.has(id) ? selection : selection.select(id);
}
/** Resolve at most `limit` URL ids; never expand a select-all to the whole crawl in the UI. */
export async function selectedIds(source: ViewDataSource, selection: Selection, limit: number): Promise<number[]> {
  if (selection.kind === "ids") return selection.ids.slice(0, limit);
  if (selection.viewId !== source.viewId || selection.version !== source.version) return [];
  const ids: number[] = [];
  const except = new Set(selection.except);
  for (let offset = 0; offset < source.total && ids.length < limit; offset += 200) {
    const rows = await view.rows({ viewId: source.viewId, version: source.version, offset, limit: 200, columns: [] });
    if (rows.version !== selection.version || source.version !== selection.version) return [];
    ids.push(...rows.rowKeys.filter(id => !except.has(id)).slice(0, limit - ids.length));
  }
  return ids;
}
export function browserUrl(value: unknown): string | null {
  if (typeof value !== "string") return null;
  try { const url = new URL(value); return url.protocol === "http:" || url.protocol === "https:" ? url.href : null; } catch { return null; }
}
export async function openSelected(source: ViewDataSource, selection: SelectionModel, confirm: () => Promise<boolean>, open: (urls: string[]) => void | Promise<void>): Promise<void> {
  const current = selection.forVersion(source.viewId, source.version);
  const count = current.count(source.total);
  if (!count || (count > 1 && !await confirm())) return;
  const version = source.version;
  const ids = await selectedIds(source, current.value, 10);
  const urls: string[] = [];
  for (const id of ids) {
    const row = await source.locate(id); if (row === null) continue;
    const result = await view.rows({ viewId: source.viewId, version, offset: row, limit: 1, columns: ["Address"] });
    if (result.version !== version || source.version !== version) return;
    const url = browserUrl(result.cells[0]?.[0]); if (url) urls.push(url);
  }
  await open(urls);
}

/** Native browser launch uses the same IPC client as the platform's other actions. */
export async function launchBrowser(urls: string[]): Promise<void> {
  const valid = urls.map(browserUrl).filter((url): url is string => url !== null).slice(0, 10);
  if (!valid.length) return;
  try {
    if (client.transportName === "tauri") await client.invoke<void>("browser_open", { urls: valid });
    else for (const url of valid) window.open(url, "_blank", "noopener,noreferrer");
  } catch (error) {
    throw new IpcError({ ...toCmdError(error), messageKey: "u10-browser-error" });
  }
}
