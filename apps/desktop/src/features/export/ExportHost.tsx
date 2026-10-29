import { useEffect, useRef, useState } from "react";
import { useT, i18n, currentLanguage } from "../../i18n";
import { exportApi } from "../../ipc/export";
import { config } from "../../ipc/config";
import { op } from "../../ipc/op";
import { describeError, toCmdError } from "../../ipc/errors";
import type { ExportCatalog } from "../../ipc/gen/ExportCatalog";
import type { ExportBatchResult } from "../../ipc/gen/ExportBatchResult";
import type { FileResult } from "../../ipc/gen/FileResult";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import type { OpStatus } from "../../ipc/gen/OpStatus";
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import { useSession } from "../../state/session";
import { registerAction, useAction } from "../../ui/actions";
import { registerMenuContributions } from "../../ui/menu/registry";
import type { MenuContribution } from "../../ui/menu/types";
import { Banner, Button, ProgressBar, ToastRegion } from "../../ui/primitives";
import { ExportDialog } from "./ExportDialog";
import type { StartInfo } from "./MultiExportDialog";
import { openExport, useExportContext, useExportDialog, type ExportRequest } from "./model";
import { nativeExportDesktop, type ExportDesktop } from "./native";
import "./export.css";

/** A reusable trigger for every tabular detail tab and sidebar panel. */
export function ExportButton({ request, disabledReason }: { request: ExportRequest | null; disabledReason?: string }) {
  const t = useT();
  const reason = disabledReason ?? (request?.kind === "detail" ? request.disabledReason : undefined);
  return <span title={reason ? t(reason) : undefined}><Button isDisabled={!request || !!reason} onPress={() => { if (request) openExport(request); }}>{t("export-trigger")}</Button></span>;
}

/** U10 publishes a snapshot via registerExportContext; Mod+E reads the same snapshot. */
export function ExportViewButton() {
  const request = useExportContext(s => s.view);
  return <ExportButton request={request} disabledReason={request ? undefined : "export-view-required"} />;
}

function resultInfo(value: JsonValue | null | undefined): { files: FileResult[]; warnings: string[]; skipped: string[] } {
  if (!value || typeof value !== "object" || Array.isArray(value)) return { files: [], warnings: [], skipped: [] };
  if (typeof value.path === "string") return { files: [value as unknown as FileResult], warnings: [], skipped: [] };
  const batch = value as unknown as ExportBatchResult;
  return { files: batch.files ?? [], warnings: batch.warnings ?? [], skipped: batch.skipped ?? [] };
}

/**
 * One export run can start several operations (multi export with local files and Looker Studio, A02-17). `known` names files
 * that an operation writes but does not report itself (`crawls.export` answers with a count): shown once it is done.
 */
export function combine(statuses: readonly (OpStatus | undefined)[], known: Readonly<Record<string, readonly string[]>> = {}): { state: OpStatus["state"] | "running"; done: number; total: number | null; errors: NonNullable<OpStatus["error"]>[]; files: FileResult[]; warnings: string[]; skipped: string[] } {
  const finished = statuses.filter((s): s is OpStatus => !!s);
  const infos = finished.map(s => {
    const info = resultInfo(s.result);
    return info.files.length || s.state !== "done" ? info : { ...info, files: (known[s.opId] ?? []).map(path => ({ path, bytes: 0 })) };
  });
  const state = statuses.some(s => !s || s.state === "running") ? "running"
    : finished.some(s => s.state === "failed") ? "failed" : finished.every(s => s.state === "cancelled") ? "cancelled" : "done";
  const totals = finished.map(s => s.total ?? null);
  return { state, done: finished.reduce((n, s) => n + s.done, 0), total: totals.every(v => v !== null) && totals.length === statuses.length ? totals.reduce<number>((n, v) => n + (v ?? 0), 0) : null,
    errors: finished.flatMap(s => s.error ? [s.error] : []), files: infos.flatMap(i => i.files), warnings: infos.flatMap(i => i.warnings), skipped: infos.flatMap(i => i.skipped) };
}

/**
 * The export catalog names its entries with `labelKey`, but for many of them the key is the catalog name itself
 * (`Accessibility:WCAG 2.0 A`): it has no translation, and Fluent would read the dot as an attribute separator (A01-22).
 * Translated keys keep their translation; the others show the name, without the group prefix the submenu already carries.
 */
export function menuLabel(key: string, groupPrefix = ""): { labelKey: string } | { label: string } {
  if (i18n.has(currentLanguage(), key)) return { labelKey: key };
  return { label: groupPrefix && key.startsWith(groupPrefix) ? key.slice(groupPrefix.length) : key };
}

/** One window-wide host; operation state survives closing the options dialog. */
export function ExportHost({ desktop = nativeExportDesktop }: { desktop?: ExportDesktop }) {
  const t = useT();
  const request = useExportDialog(s => s.request);
  const [active, setActive] = useState<{ handles: OpHandle[]; path: string; info?: StartInfo } | null>(null);
  const ops = useSession(s => s.ops);
  const status = active ? combine(active.handles.map(h => ops[h.opId]), active.info?.files) : undefined;
  const crawlId = useSession(s => s.crawl?.crawlId);
  const countsVersion = useSession(s => s.countsVersion);
  const latestCounts = useRef(countsVersion);
  latestCounts.current = countsVersion;
  const [exports, setExports] = useState<ExportCatalog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const running = !!active && status?.state === "running";
  const errorText = (e: unknown) => describeError(t, toCmdError(e), key => i18n.has(currentLanguage(), key)).text;
  useAction("export.view", () => {
    const view = useExportContext.getState().view;
    if (!view || running) return false;
    openExport(view);
  });
  useAction("export.detail", () => {
    const detail = useExportContext.getState().detail;
    if (!detail || detail.disabledReason || running) return false;
    openExport(detail);
  });
  useAction("export.sidebar", () => {
    const sidebar = useExportContext.getState().sidebar;
    if (!sidebar || running) return false;
    openExport(sidebar);
  });
  useAction("export.multi", () => { if (!running) openExport({ kind: "multi" }); });
  useEffect(() => {
    let live = true;
    let pending = false;
    let requested: number | undefined;
    setExports(null);
    // Preserve the last catalog while a single refresh catches up. Starting a
    // request on every tick used to fill the core's read pool with obsolete work.
    const refresh = async () => {
      if (!live || !crawlId || pending || requested === latestCounts.current) return;
      pending = true; requested = latestCounts.current;
      try { const value = await exportApi.catalog(); if (live) setExports(previous => JSON.stringify(previous) === JSON.stringify(value) ? previous : value); }
      catch { /* The next version retries; no crawl has no export catalog. */ }
      finally { pending = false; }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 500);
    return () => { live = false; clearInterval(timer); };
  }, [crawlId]);
  useEffect(() => {
    const offs: (() => void)[] = [];
    const menu: MenuContribution[] = [{ menu: "bulkExport", group: "multi", order: 0, items: [{ id: "export.multi", action: "export.multi", labelKey: "export-multi", enabled: c => c.hasCrawl === true && !running }] }];
    for (const key of ["bulk", "reports"] as const) {
      for (const group of exports?.[key] ?? []) {
        const kind = key === "bulk" ? "bulk" : "report";
        const items = group.items.map(item => {
          const action = `export.${kind}.${item.name}`;
          offs.push(registerAction(action, () => { if (item.enabled && !running) openExport({ kind, name: item.name }); }));
          return { id: action, action, ...menuLabel(item.labelKey, `${group.key}:`), enabled: () => item.enabled && !running,
            // The catalog's reason is a key or, for many reports, plain English text: untranslated reasons get the generic text.
            disabledReasonKey: item.disabledReason ? (i18n.has(currentLanguage(), item.disabledReason) ? item.disabledReason : "export-unavailable") : undefined };
        });
        menu.push({ menu: key === "bulk" ? "bulkExport" : "reports", group: `export-${group.key}`, order: 10, items: [{ id: `export-group-${key}-${group.key}`, ...menuLabel(group.labelKey), items }] });
      }
    }
    for (const sitemapKind of ["pages", "images"] as const) {
      const action = `export.sitemap.${sitemapKind}`;
      offs.push(registerAction(action, async () => {
        if (running) return;
        try {
          const dto = await config.get();
          const root = dto.config as { mode?: string; spider?: { hreflang?: { store?: boolean } } };
          openExport({ kind: "sitemap", sitemapKind, listMode: root.mode === "list", hreflangAvailable: root.spider?.hreflang?.store === true });
        } catch (e) { setError(errorText(e)); }
      }));
      menu.push({ menu: "sitemaps", group: "export-sitemaps", order: 10, items: [{ id: action, action, labelKey: `export-sitemap-${sitemapKind}`, enabled: c => c.hasCrawl === true && !running }] });
    }
    offs.push(registerMenuContributions("features/export", menu));
    return () => { for (const off of offs.reverse()) off(); };
  }, [exports, running, t]);
  const close = () => useExportDialog.setState({ request: null });
  return <>
    {request && !running && <ExportDialog key={JSON.stringify(request)} request={request} desktop={desktop} onClose={close} onStarted={(handles, path, info) => { setActive({ handles, path, info }); setCancelling(false); setError(info?.errors.length ? info.errors.join(" ") : null); close(); }} />}
    <div className="os-export-notifications"><ToastRegion label={t("export-notifications")}>
      {active && status && running && <Banner action={active.handles.some(h => h.cancellable) ? <Button isDisabled={cancelling} onPress={() => {
        setCancelling(true);
        for (const handle of active.handles.filter(h => h.cancellable && ops[h.opId]?.state !== "done")) void op.cancel({ opId: handle.opId }).catch(e => { setCancelling(false); setError(errorText(e)); });
      }}>{t(cancelling ? "export-cancelling" : "ui-dialog-cancel")}</Button> : undefined}>
        <ProgressBar label={t("export-progress")} value={status.total ? status.done : null} max={status.total || 100} />
      </Banner>}
      {active && status && !running && <Banner kind={status.state === "failed" ? "error" : "info"} onDismiss={() => setActive(null)} dismissLabel={t("ui-dialog-close")}
        action={status.files.length > 0 ? <Button onPress={() => void desktop.reveal(status.files[0]!.path).catch(e => setError(errorText(e)))}>{t("export-open-folder")}</Button> : undefined}>
        {status.state === "failed" ? status.errors.length ? status.errors.map(errorText).join(" ") : t("export-failed") : t(status.state === "cancelled" ? "export-cancelled" : "export-complete")}
        {active.handles.length > 1 && status.files.length > 0 && <p className="os-export-result-head">{t("export-files-written", { count: status.files.length })}</p>}
        {status.files.map(file => <p key={file.path}>{file.path}</p>)}
        {status.warnings.map((warning, i) => <p key={`warning-${i}`}>{warning}</p>)}
        {status.skipped.length > 0 && <p>{t("export-skipped-warning")} {status.skipped.join(", ")}</p>}
      </Banner>}
      {error && <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>{error}</Banner>}
    </ToastRegion></div>
  </>;
}
