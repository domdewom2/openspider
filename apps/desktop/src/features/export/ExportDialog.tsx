import { useCallback, useEffect, useRef, useState } from "react";
import { useT, i18n, currentLanguage } from "../../i18n";
import { config } from "../../ipc/config";
import { exportApi } from "../../ipc/export";
import { describeError, toCmdError } from "../../ipc/errors";
import type { OpHandle } from "../../ipc/gen/OpHandle";
import { usePrefs } from "../../state/prefs";
import { Button, Checkbox, Dialog, DialogFooter, Select } from "../../ui/primitives";
import { executeExport, type ExportChoices } from "./execute";
import { nameTarget, type ExportRequest, type LocalFormat } from "./model";
import { nativeExportDesktop, type ExportDesktop } from "./native";
import { defaultSitemapOptions, sitemapSchema } from "./sitemap";
import { SitemapOptions } from "./SitemapOptions";
import { MultiExportDialog } from "./MultiExportDialog";

export function ExportDialog(props: {
  request: ExportRequest; onClose(): void; onStarted(handles: OpHandle[], path: string): void; desktop?: ExportDesktop;
}) {
  // OUT-075 (A02-17): the multi export has a dialog of its own with output, selection, local files, Looker Studio and presets.
  return props.request.kind === "multi" ? <MultiExportDialog onClose={props.onClose} onStarted={props.onStarted} desktop={props.desktop} /> : <SingleExportDialog {...props} />;
}

function SingleExportDialog({ request, onClose, onStarted, desktop = nativeExportDesktop }: {
  request: ExportRequest; onClose(): void; onStarted(handles: OpHandle[], path: string): void; desktop?: ExportDesktop;
}) {
  const t = useT();
  const prefs = usePrefs(s => s.dto?.export);
  const [choices, setChoices] = useState<ExportChoices>(() => ({ format: prefs?.format === "xlsx" ? "xlsx" : "csv", allColumns: false,
    preserveUploadOrder: request.kind === "view" ? request.preserveUploadOrder !== false : true,
    singleWorkbook: false, skipEmpty: false, items: [], sitemap: defaultSitemapOptions() }));
  const [ready, setReady] = useState(request.kind !== "multi" && request.kind !== "sitemap");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [attempt, setAttempt] = useState(0);
  const lock = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const showError = useCallback((e: unknown) => {
    if (!mounted.current) return;
    setError(describeError(t, toCmdError(e), key => i18n.has(currentLanguage(), key)).text);
  }, [t]);
  useEffect(() => {
    if (request.kind !== "sitemap") return;
    let live = true;
    void config.get().then(dto => {
      if (!live) return;
      const root = dto.config as Record<string, unknown>;
      const sitemap = sitemapSchema.parse(root.sitemap_gen ?? {});
      if (request.sitemapKind === "images") sitemap.images.enabled = true;
      if (!request.hreflangAvailable) sitemap.hreflang.enabled = false;
      setChoices(v => ({ ...v, sitemap }));
      setReady(true);
    }).catch(e => { if (live) showError(e); });
    return () => { live = false; };
  }, [request, showError, attempt]);
  useEffect(() => {
    let live = true;
    const target = nameTarget(request, choices.format);
    setName("");
    if (target) void exportApi.suggestName({ target }).then(n => { if (live) setName(n); }).catch(e => { if (live) showError(e); });
    else setName("multi");
    return () => { live = false; };
  }, [request, choices.format, showError, attempt]);
  const folder = request.kind === "bulk" || request.kind === "report" || request.kind === "multi" || request.kind === "sitemap";
  const valid = request.kind !== "sitemap" || sitemapSchema.safeParse(choices.sitemap).success;
  const disabled = request.kind === "detail" && !!request.disabledReason;
  const start = async () => {
    if (lock.current || !ready || !valid || disabled || !name || (request.kind === "multi" && !choices.items.length)) return;
    lock.current = true; setBusy(true); setError(null);
    try {
      const path = await desktop.choose({ title: t(folder ? "export-choose-folder" : "export-save"), defaultName: name, directory: prefs?.directory ?? null,
        extension: request.kind === "sitemap" ? "xml" : choices.format, folder });
      if (!path || !mounted.current) return;
      const directory = await desktop.directory(path, folder);
      const handle = await executeExport(request, path, choices);
      await usePrefs.getState().patch({ export: { directory, ...(request.kind === "sitemap" ? {} : { format: choices.format }) } });
      onStarted([handle], path);
      onClose();
    } catch (e) { showError(e); }
    finally { lock.current = false; if (mounted.current) setBusy(false); }
  };
  return <Dialog title={t(request.kind === "sitemap" ? "export-sitemap-title" : "export-title")} isOpen onOpenChange={open => { if (!open && !lock.current) onClose(); }} isDismissable={!busy} size="large"
    footer={<DialogFooter okLabel={t("export-save")} cancelLabel={t("ui-dialog-cancel")} okDisabled={busy || !ready || !name || !valid || disabled || (request.kind === "multi" && !choices.items.length)} onOk={() => void start()} onCancel={() => { if (!lock.current) onClose(); }} />}>
    <div className="os-export-fields" aria-busy={busy}>
      {request.kind !== "sitemap" && <Select label={t("export-format")} value={choices.format} options={[{ value: "csv", label: "CSV" }, { value: "xlsx", label: "Excel Workbook (.xlsx)" }]} isDisabled={busy}
        onChange={format => setChoices(v => ({ ...v, format: format as LocalFormat }))} />}
      {request.kind === "view" && <>
        <Checkbox isSelected={choices.allColumns} onChange={allColumns => setChoices(v => ({ ...v, allColumns }))}>{t("export-all-columns")}</Checkbox>
        <Checkbox isSelected={choices.preserveUploadOrder} onChange={preserveUploadOrder => setChoices(v => ({ ...v, preserveUploadOrder }))}>{t("export-upload-order")}</Checkbox>
      </>}
      {(request.kind === "multi" || request.kind === "bulk" || request.kind === "report") && <Checkbox isDisabled={choices.format !== "xlsx"} isSelected={choices.format === "xlsx" && choices.singleWorkbook} onChange={singleWorkbook => setChoices(v => ({ ...v, singleWorkbook }))}>{t("export-workbook")}</Checkbox>}
      {request.kind === "sitemap" && <SitemapOptions value={choices.sitemap} onChange={sitemap => setChoices(v => ({ ...v, sitemap }))} listMode={request.listMode} hreflangAvailable={request.hreflangAvailable} />}
      {!valid && <p role="alert">{t("export-sitemap-invalid")}</p>}
      {disabled && <p role="note">{t(request.disabledReason!)}</p>}
      {name && <p className="os-export-filename">{t("export-filename", { name })}</p>}
      {error && <div role="alert">{error}<Button onPress={() => { setError(null); if (!ready || !name) setAttempt(n => n + 1); else void start(); }}>{t("export-retry")}</Button></div>}
    </div>
  </Dialog>;
}
