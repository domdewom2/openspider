import { create } from "zustand";
import type { ExportViewArgs } from "../../ipc/gen/ExportViewArgs";
import type { ExportDetailArgs } from "../../ipc/gen/ExportDetailArgs";
import type { ExportTarget } from "../../ipc/gen/ExportTarget";
import type { PanelKind } from "../../ipc/gen/PanelKind";

export type LocalFormat = "csv" | "xlsx";
export type ExportRequest =
  | ({ kind: "view"; tab: string; filter: string } & Omit<ExportViewArgs, "path" | "format" | "allColumns">)
  | ({ kind: "detail"; disabledReason?: string } & Omit<ExportDetailArgs, "path" | "format">)
  | { kind: "sidebar" | "issues"; which: PanelKind }
  | { kind: "bulk" | "report"; name: string }
  | { kind: "multi" }
  | { kind: "sitemap"; sitemapKind: "pages" | "images"; listMode: boolean; hreflangAvailable: boolean };

/** Each region publishes its exact snapshot when it becomes active. No row data lives here. */
export const useExportContext = create<{
  view: Extract<ExportRequest, { kind: "view" }> | null;
  detail: Extract<ExportRequest, { kind: "detail" }> | null;
  sidebar: Extract<ExportRequest, { kind: "sidebar" | "issues" }> | null;
}>(() => ({ view: null, detail: null, sidebar: null }));

/** Called by table/detail/sidebar owners; dispose cannot clear a newer region's context. */
export function registerExportContext<K extends "view" | "detail" | "sidebar">(region: K, request: NonNullable<ReturnType<typeof useExportContext.getState>[K]>): () => void {
  useExportContext.setState({ [region]: request });
  return () => {
    if (useExportContext.getState()[region] === request) useExportContext.setState({ [region]: null });
  };
}

export const useExportDialog = create<{ request: ExportRequest | null }>(() => ({ request: null }));
export function openExport(request: ExportRequest): void {
  // Snapshot the caller's selection/columns; subsequent UI changes must not change the export.
  useExportDialog.setState({ request: structuredClone(request) });
}

export function nameTarget(request: ExportRequest, format: LocalFormat): ExportTarget | null {
  switch (request.kind) {
    case "view": return { kind: "view", tab: request.tab, filter: request.filter, format };
    case "detail": return { kind: "detail", spec: request.spec, format };
    case "bulk": case "report": return { kind: request.kind, name: request.name, format };
    case "sitemap": return { kind: "sitemap", sitemapKind: request.sitemapKind };
    case "sidebar": case "issues": return { kind: "report", name: request.which, format };
    case "multi": return null;
  }
}
