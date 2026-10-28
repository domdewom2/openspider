import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useT, i18n, currentLanguage } from "../../i18n";
import { api } from "../../ipc/api";
import { config } from "../../ipc/config";
import { crawls } from "../../ipc/crawls";
import { exportApi } from "../../ipc/export";
import { describeError, toCmdError } from "../../ipc/errors";
import type { ApiAccount } from "../../ipc/gen/ApiAccount";
import type { CustomSummaryMetric } from "../../ipc/gen/CustomSummaryMetric";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import { usePrefs } from "../../state/prefs";
import { useSession } from "../../state/session";
import { Button, Checkbox, Dialog, DialogFooter, Select, TextField } from "../../ui/primitives";
import type { ExportChoices } from "./execute";
import type { LocalFormat } from "./model";
import { MultiOptions, PresetControls } from "./MultiOptions";
import { nativeExportDesktop, type ExportDesktop } from "./native";
import { defaultSitemapOptions, sitemapSchema } from "./sitemap";

export type LocalFile = "crawl" | "sitemap" | "imageSitemap";
export interface MultiExtras {
  /** Local folder, empty = asked for on export. */
  dir: string;
  timestamp: boolean;
  drive: boolean;
  account: string;
  sheetName: string;
  files: LocalFile[];
  looker: boolean;
  lookerTarget: string;
  metrics: string[];
}
export interface Missing { field: "selection" | "account" | "sheetName" | "lookerTarget" | "metrics"; key: string }

/** Required inputs that block the export (A02-17): named at the field and counted in the footer. */
export function missingInputs(items: number, extras: MultiExtras): Missing[] {
  const missing: Missing[] = [];
  if (!items && !extras.files.length && !extras.looker) missing.push({ field: "selection", key: "export-required-selection" });
  if ((extras.drive || extras.looker) && !extras.account) missing.push({ field: "account", key: "export-required-account" });
  if (extras.drive && items && !extras.sheetName.trim()) missing.push({ field: "sheetName", key: "export-required-sheet" });
  if (extras.looker && !extras.lookerTarget.trim()) missing.push({ field: "lookerTarget", key: "export-required-target" });
  if (extras.looker && !extras.metrics.length) missing.push({ field: "metrics", key: "export-required-metrics" });
  return missing;
}

/** Joins a name to a folder chosen by the user; the separator follows that path (Windows `C:\…` or POSIX). */
export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return `${dir.replace(/[\\/]+$/, "")}${sep}${name}`;
}

/** OUT-013: a timestamped subfolder keeps earlier exports (local time of the computer). */
export function timestampFolder(dir: string, now: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return joinPath(dir, `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}_${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`);
}

/** What the export status needs besides the operations: files known before the result (crawl copy) and start errors. */
export interface StartInfo { files: Record<string, string[]>; errors: string[] }

/**
 * Multi export after OUT-075 (A02-17): output – selection – local files (OUT-165) – Looker Studio time series (OUT-210) – presets.
 * One click starts every chosen export; the export status lists the files written and the entries skipped.
 */
export function MultiExportDialog({ onClose, onStarted, desktop = nativeExportDesktop }: {
  onClose(): void; onStarted(handles: OpHandle[], path: string, info?: StartInfo): void; desktop?: ExportDesktop;
}) {
  const t = useT();
  const prefs = usePrefs(s => s.dto?.export);
  const crawlId = useSession(s => s.crawl?.crawlId);
  const [choices, setChoices] = useState<ExportChoices>(() => ({ format: prefs?.format === "xlsx" ? "xlsx" : "csv", allColumns: false, preserveUploadOrder: true,
    singleWorkbook: false, skipEmpty: false, items: [], sitemap: defaultSitemapOptions() }));
  const [extras, setExtras] = useState<MultiExtras>(() => ({ dir: prefs?.directory ?? "", timestamp: false, drive: false, account: "", sheetName: "openspider-export",
    files: [], looker: false, lookerTarget: "", metrics: [] }));
  const [accounts, setAccounts] = useState<ApiAccount[]>([]);
  const [metrics, setMetrics] = useState<CustomSummaryMetric[]>([]);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const showError = useCallback((e: unknown) => {
    if (mounted.current) setError(describeError(t, toCmdError(e), key => i18n.has(currentLanguage(), key)).text);
  }, [t]);
  useEffect(() => {
    let live = true;
    // Drive is optional: without a connected account Drive and Looker Studio stay disabled with a hint, nothing fails.
    void api.list().then(sources => { if (live) setAccounts(sources.find(s => s.kind === "drive")?.accounts ?? []); }).catch(() => undefined);
    void exportApi.customSummaryMetrics().then(list => { if (live) setMetrics(list); }).catch(() => undefined);
    return () => { live = false; };
  }, []);
  useEffect(() => { if (accounts.length && !extras.account) setExtras(v => ({ ...v, account: accounts[0]!.account })); }, [accounts]);
  const patch = (next: Partial<MultiExtras>) => setExtras(v => ({ ...v, ...next }));
  const missing = useMemo(() => missingInputs(choices.items.length, extras), [choices.items.length, extras]);
  const message = (field: Missing["field"]) => { const m = missing.find(x => x.field === field); return m ? t(m.key) : undefined; };
  const driveAvailable = accounts.length > 0;
  const chooseFolder = async () => {
    try {
      const path = await desktop.choose({ title: t("export-choose-folder"), defaultName: "", directory: extras.dir || prefs?.directory || null, extension: choices.format, folder: true });
      if (path && mounted.current) patch({ dir: path });
    } catch (e) { showError(e); }
  };
  const start = async () => {
    if (lock.current || !ready || missing.length) return;
    lock.current = true; setBusy(true); setError(null);
    try {
      const local = choices.items.length > 0 || extras.files.length > 0;
      let base = extras.dir;
      if (local && !base) {
        base = await desktop.choose({ title: t("export-choose-folder"), defaultName: "multi", directory: prefs?.directory ?? null, extension: choices.format, folder: true }) ?? "";
        if (!base || !mounted.current) return;
      }
      const dir = local && extras.timestamp ? timestampFolder(base, new Date()) : base;
      // Every started operation is handed to the export status, even if a later start fails (A02-17 review): a failed step is
      // reported next to the running exports instead of hiding them; nothing that already runs is started twice.
      const handles: OpHandle[] = [];
      const info: StartInfo = { files: {}, errors: [] };
      const run = async (job: () => Promise<OpHandle | OpHandle[] | undefined>, files?: string[]) => {
        try {
          const started = await job();
          for (const handle of started === undefined ? [] : Array.isArray(started) ? started : [started]) {
            handles.push(handle);
            if (files) info.files[handle.opId] = files;
          }
        } catch (e) { info.errors.push(describeError(t, toCmdError(e), key => i18n.has(currentLanguage(), key)).text); }
      };
      const singleWorkbook = choices.format === "xlsx" && choices.singleWorkbook;
      if (choices.items.length) await run(() => exportApi.multi({ items: choices.items, format: choices.format, dir, singleWorkbook, skipEmpty: choices.skipEmpty }));
      if (extras.drive && choices.items.length) await run(() => exportApi.sheets({ account: extras.account, selections: choices.items, name: extras.sheetName.trim() }));
      if (extras.files.some(f => f !== "crawl")) await run(async () => {
        const dto = await config.get();
        const root = dto.config as { sitemap_gen?: unknown; spider?: { hreflang?: { store?: boolean } } };
        for (const kind of ["sitemap", "imageSitemap"] as const) {
          if (!extras.files.includes(kind)) continue;
          const options = sitemapSchema.parse(root.sitemap_gen ?? {});
          if (kind === "imageSitemap") options.images.enabled = true;
          if (root.spider?.hreflang?.store !== true) options.hreflang.enabled = false;
          await run(() => exportApi.sitemap({ sitemapKind: kind === "sitemap" ? "pages" : "images", options, dir }));
        }
        return undefined;
      });
      if (extras.files.includes("crawl") && crawlId) {
        // A single id with a path that does not exist yet is taken as the file name by `crawls.export`: name the file itself
        // (the core creates missing folders), so a new timestamped folder gets `<id>.ospider` inside.
        const target = joinPath(dir, `${crawlId}.ospider`);
        await run(() => crawls.export({ ids: [crawlId], dest: target }), [target]);
      }
      if (extras.looker) await run(() => exportApi.customSummary({ account: extras.account, target: extras.lookerTarget.trim(), metrics: extras.metrics }));
      if (!handles.length) { if (mounted.current) setError(info.errors.join(" ")); return; }
      if (local) await usePrefs.getState().patch({ export: { directory: base, format: choices.format } })
        .catch(e => { info.errors.push(describeError(t, toCmdError(e), key => i18n.has(currentLanguage(), key)).text); });
      onStarted(handles, dir, info);
      onClose();
    } catch (e) { showError(e); }
    finally { lock.current = false; if (mounted.current) setBusy(false); }
  };
  const toggleFile = (file: LocalFile, checked: boolean) => patch({ files: checked ? [...extras.files, file] : extras.files.filter(f => f !== file) });
  const metricLabel = (m: string) => { const found = metrics.find(x => x.id === m); return found && i18n.has(currentLanguage(), found.labelKey) ? t(found.labelKey) : m; };
  return <Dialog title={t("export-title")} isOpen onOpenChange={open => { if (!open && !lock.current) onClose(); }} isDismissable={!busy} size="large"
    footer={<DialogFooter okLabel={t("export-run")} cancelLabel={t("ui-dialog-cancel")} okDisabled={busy || !ready || missing.length > 0} onOk={() => void start()} onCancel={() => { if (!lock.current) onClose(); }}
      extra={missing.length > 0 ? <span className="os-export-missing" role="status">{t("export-missing", { count: missing.length })}</span> : undefined} />}>
    <div className="os-export-fields os-export-multi" aria-busy={busy}>
      <section className="os-export-section" aria-labelledby="os-export-output"><h3 id="os-export-output">{t("export-section-output")}</h3>
        <div className="os-export-row">
          <TextField label={t("export-folder")} value={extras.dir} onChange={dir => patch({ dir })} description={extras.dir ? undefined : t("export-folder-ask")} />
          <Button onPress={() => void chooseFolder()}>{t("export-folder-choose")}</Button>
        </div>
        <Checkbox isSelected={extras.timestamp} onChange={timestamp => patch({ timestamp })}>{t("export-timestamp")}</Checkbox>
        <Select label={t("export-format")} value={choices.format} options={[{ value: "csv", label: "CSV" }, { value: "xlsx", label: "Excel Workbook (.xlsx)" }]} isDisabled={busy}
          onChange={format => setChoices(v => ({ ...v, format: format as LocalFormat }))} />
        <Checkbox isDisabled={choices.format !== "xlsx"} isSelected={choices.format === "xlsx" && choices.singleWorkbook} onChange={singleWorkbook => setChoices(v => ({ ...v, singleWorkbook }))}>{t("export-workbook")}</Checkbox>
        <Checkbox isSelected={choices.skipEmpty} onChange={skipEmpty => setChoices(v => ({ ...v, skipEmpty }))}>{t("export-skip-empty")}</Checkbox>
        <Checkbox isDisabled={!driveAvailable} isSelected={extras.drive && driveAvailable} onChange={drive => patch({ drive })}>{t("export-drive")}</Checkbox>
        {!driveAvailable && <p className="os-export-hint">{t("export-drive-unavailable")}</p>}
        {(extras.drive || extras.looker) && driveAvailable && <Select label={t("export-drive-account")} value={extras.account} onChange={account => patch({ account })}
          options={accounts.map(a => ({ value: a.account, label: a.account }))} />}
        {message("account") && <p className="os-export-required" role="alert">{message("account")}</p>}
        {extras.drive && driveAvailable && <TextField label={t("export-sheet-name")} value={extras.sheetName} onChange={sheetName => patch({ sheetName })} error={message("sheetName")} />}
      </section>
      <section className="os-export-section" aria-labelledby="os-export-selection"><h3 id="os-export-selection">{t("export-section-selection")}</h3>
        <MultiOptions value={choices} onChange={setChoices} onReady={setReady} onError={showError} showPresets={false} showSkipEmpty={false} />
        {message("selection") && <p className="os-export-required" role="alert">{message("selection")}</p>}
      </section>
      <section className="os-export-section" aria-labelledby="os-export-files"><h3 id="os-export-files">{t("export-section-files")}</h3>
        <Checkbox isDisabled={!crawlId} isSelected={extras.files.includes("crawl")} onChange={checked => toggleFile("crawl", checked)}>{t("export-file-crawl")}</Checkbox>
        <Checkbox isSelected={extras.files.includes("sitemap")} onChange={checked => toggleFile("sitemap", checked)}>{t("export-file-sitemap")}</Checkbox>
        <Checkbox isSelected={extras.files.includes("imageSitemap")} onChange={checked => toggleFile("imageSitemap", checked)}>{t("export-file-image-sitemap")}</Checkbox>
      </section>
      <section className="os-export-section" aria-labelledby="os-export-looker"><h3 id="os-export-looker">{t("export-section-looker")}</h3>
        <Checkbox isDisabled={!driveAvailable} isSelected={extras.looker && driveAvailable} onChange={looker => patch({ looker })}>{t("export-looker")}</Checkbox>
        {!driveAvailable && <p className="os-export-hint">{t("export-looker-unavailable")}</p>}
        {extras.looker && driveAvailable && <>
          <TextField label={t("export-looker-target")} value={extras.lookerTarget} onChange={lookerTarget => patch({ lookerTarget })} error={message("lookerTarget")} />
          <div className="os-export-picker">
            <div className="os-export-pane" role="group" aria-label={t("export-metrics-available")}>
              <span className="os-export-pane-title">{t("export-metrics-available")}</span>
              {metrics.filter(m => !extras.metrics.includes(m.id)).map(m => <Button key={m.id} onPress={() => patch({ metrics: [...extras.metrics, m.id] })}>{metricLabel(m.id)}</Button>)}
            </div>
            <div className="os-export-pane">
              <span className="os-export-pane-title" id="os-export-metrics-selected">{t("export-metrics-selected")}</span>
              <ol className="os-export-selected" aria-labelledby="os-export-metrics-selected">{extras.metrics.map((m, i) => <li key={m}><span>{metricLabel(m)}</span>
                <Button aria-label={t("export-move-up", { name: metricLabel(m) })} isDisabled={i === 0} onPress={() => { const next = [...extras.metrics]; [next[i - 1], next[i]] = [next[i]!, next[i - 1]!]; patch({ metrics: next }); }}>↑</Button>
                <Button aria-label={t("export-remove", { name: metricLabel(m) })} onPress={() => patch({ metrics: extras.metrics.filter(x => x !== m) })}>×</Button></li>)}</ol>
              {message("metrics") && <p className="os-export-required" role="alert">{message("metrics")}</p>}
            </div>
          </div>
        </>}
      </section>
      <section className="os-export-section" aria-labelledby="os-export-presets"><h3 id="os-export-presets">{t("export-section-presets")}</h3>
        <PresetControls value={choices} onChange={setChoices} onError={showError} />
      </section>
      {error && <div role="alert">{error}</div>}
    </div>
  </Dialog>;
}
