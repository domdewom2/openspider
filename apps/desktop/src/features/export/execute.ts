import { exportApi } from "../../ipc/export";
import { config } from "../../ipc/config";
import type { ExportSel } from "../../ipc/gen/ExportSel";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import type { ExportRequest, LocalFormat } from "./model";
import { sitemapSchema, type SitemapOptions } from "./sitemap";

export interface ExportChoices {
  format: LocalFormat;
  allColumns: boolean;
  preserveUploadOrder: boolean;
  singleWorkbook: boolean;
  skipEmpty: boolean;
  items: ExportSel[];
  sitemap: SitemapOptions;
}

/** The UI passes snapshot IDs and selections, never pages/rows or an approximated filter. */
export async function executeExport(request: ExportRequest, path: string, choices: ExportChoices): Promise<OpHandle> {
  const { format } = choices;
  const singleWorkbook = format === "xlsx" && choices.singleWorkbook;
  switch (request.kind) {
    case "view": return exportApi.view({ viewId: request.viewId, version: request.version, path, format, allColumns: choices.allColumns,
      columns: choices.allColumns ? null : request.columns, preserveUploadOrder: choices.preserveUploadOrder });
    case "detail": return exportApi.detail({ spec: request.spec, selection: request.selection, path, format });
    case "sidebar": return exportApi.sidebar({ which: request.which, path, format });
    case "issues": return exportApi.issues({ which: request.which, path, format });
    case "bulk": case "report": return exportApi[request.kind]({ name: request.name, dir: path, format, singleWorkbook });
    case "multi": return exportApi.multi({ items: choices.items, format, dir: path, singleWorkbook, skipEmpty: choices.skipEmpty });
    case "sitemap": {
      const options = sitemapSchema.parse(choices.sitemap);
      // A crawl with extraction disabled must never emit previously saved hreflang settings.
      if (!request.hreflangAvailable) options.hreflang.enabled = false;
      const current = await config.get();
      const existing = current.config as Record<string, JsonValue>;
      await config.set({ ...current, config: { ...existing, sitemap_gen: options } });
      return exportApi.sitemap({ sitemapKind: request.sitemapKind, options, dir: path });
    }
  }
}
