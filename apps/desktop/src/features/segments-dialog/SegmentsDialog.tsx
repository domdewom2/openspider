import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useT, useFormat } from "../../i18n";
import { catalog } from "../../ipc/catalog";
import { segment as segmentApi } from "../../ipc/segment";
import type { SegmentDef } from "../../ipc/gen/SegmentDef";
import { registerAction } from "../../ui/actions";
import { Dialog, DialogFooter } from "../../ui/primitives/Dialog";
import { SEGMENT_PALETTE, segmentLabelColor } from "../../ui/tokens/tokens";
import { isHexColor } from "../../ui/tokens/color";
import { QueryFeedback } from "../sidebar/frame/QueryFeedback";
import { RuleEditor } from "./RuleEditor";
import { SEGMENT_COLUMN, exportJson, hasBlockingError, importJson, issueTarget, move, nameError, newSegment, pendingColumns, renumber } from "./model";
import { useSegmentPreview, useSegmentValidation } from "./hooks";
import { closeSegmentsDialog, openSegmentsDialog, useSegmentsDialog } from "./store";
import "./segments-dialog.css";

/** Mounted once per main window: registers the action and holds the dialog (UI-068). */
export function SegmentsDialogHost() {
  const open = useSegmentsDialog(s => s.open);
  useEffect(() => registerAction("segments.configure", () => { openSegmentsDialog(); }), []);
  return open ? <SegmentsDialog /> : null;
}

const SWATCHES = SEGMENT_PALETTE.map(c => c.light);

export function SegmentsDialog() {
  const t = useT();
  const f = useFormat();
  const queryClient = useQueryClient();
  const [segments, setSegments] = useState<SegmentDef[] | null>(null);
  const [loadError, setLoadError] = useState<unknown>();
  const [selected, setSelected] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<unknown>();
  const [drag, setDrag] = useState<number | null>(null);
  const [json, setJson] = useState("");
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [columns, setColumns] = useState<{ names: string[]; pending: Set<string> }>({ names: [SEGMENT_COLUMN], pending: new Set() });
  const [hex, setHex] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void segmentApi.list().then(list => {
      if (!live) return;
      const sorted = renumber([...list].sort((a, b) => a.order - b.order));
      setSegments(sorted);
      setSelected(sorted[0]?.id ?? null);
    }).catch(e => { if (live) setLoadError(e); });
    void catalog.columns({ tab: "Internal" }).then(cols => {
      if (live) setColumns({ names: [...new Set([...cols.map(c => c.key), SEGMENT_COLUMN])], pending: pendingColumns(cols.map(c => ({ key: c.key, needs: c.needs ?? null }))) });
    }).catch(() => undefined);
    return () => { live = false; };
  }, []);

  const list = useMemo(() => segments ?? [], [segments]);
  const validation = useSegmentValidation(list, segments !== null);
  const preview = useSegmentPreview(list, segments !== null);
  const index = list.findIndex(s => s.id === selected);
  const current = index >= 0 ? list[index] : undefined;
  const invalid = list.some((_, i) => hasBlockingError(list, i, validation.issuesOf(list[i]!)));
  const canSave = segments !== null && !invalid && !validation.pending && !validation.error && !saving;

  const update = (id: string, patch: Partial<SegmentDef>) => setSegments(cur => cur && cur.map(s => (s.id === id ? { ...s, ...patch } : s)));
  const add = () => {
    const created = newSegment(list);
    setSegments([...list, created]);
    setSelected(created.id);
    setHex(null);
  };
  const remove = (id: string) => {
    const at = list.findIndex(s => s.id === id);
    const next = renumber(list.filter(s => s.id !== id));
    setSegments(next);
    if (selected === id) setSelected(next[Math.min(at, next.length - 1)]?.id ?? null);
  };
  const shift = (from: number, to: number) => setSegments(cur => cur && move(cur, from, to));

  async function save(close: () => void) {
    if (!segments) return;
    setSaving(true);
    setSaveError(undefined);
    try {
      await segmentApi.save({ segments: renumber(segments) });
      await segmentApi.reassign();
      void queryClient.invalidateQueries({ queryKey: ["sidebar"] });
      close();
    } catch (e) { setSaveError(e); } finally { setSaving(false); }
  }

  function applyImport() {
    const result = importJson(json);
    if (!result.ok) { setJsonError(result.error); return; }
    setJsonError(null);
    setSegments(result.segments);
    setSelected(result.segments[0]?.id ?? null);
  }
  function doExport() {
    const text = exportJson(list);
    setJson(text);
    setJsonError(null);
    void navigator.clipboard?.writeText(text).catch(() => undefined);
  }
  async function loadFile(file: File | undefined) {
    if (file) { setJson(await file.text()); setJsonError(null); }
  }

  const nameMessage = current ? nameError(list, index) : null;
  const colorValue = hex ?? current?.color ?? "";
  return <Dialog title={t("sgd-title")} isOpen onOpenChange={open => { if (!open) closeSegmentsDialog(); }} size="large" isDismissable={false}
    footer={close => <DialogFooter okLabel={t("ui-dialog-ok")} cancelLabel={t("ui-dialog-cancel")} okDisabled={!canSave} onOk={() => void save(() => { close(); closeSegmentsDialog(); })} onCancel={() => { close(); closeSegmentsDialog(); }} />}>
    <div className="os-segdlg" data-segments-dialog>
      <QueryFeedback error={loadError ?? saveError ?? validation.error} loading={segments === null && !loadError} />
      {segments && <>
        <div className="os-segdlg-cols">
          <div className="os-segdlg-list">
            <ul aria-label={t("sgd-list")}>{list.map((s, i) => {
              const count = preview.counts.get(s.id);
              return <li key={s.id} draggable data-selected={s.id === selected || undefined} data-segment-id={s.id}
                onDragStart={() => setDrag(i)} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); if (drag != null) shift(drag, i); setDrag(null); }}>
                <button type="button" className="os-segdlg-pick" aria-current={s.id === selected} onClick={() => { setSelected(s.id); setHex(null); }}>
                  <span className="os-segdlg-swatch" style={{ background: s.color }} aria-hidden="true" />
                  <span className="os-segdlg-name">{s.name || t("sgd-unnamed")}</span>
                  <span className="os-segdlg-count" data-preview-count aria-busy={preview.busy || undefined}>{count == null ? "–" : t("sgd-matches", { count: f.integer(count) })}</span>
                </button>
                <button type="button" className="os-btn os-btn-tool" aria-label={t("sgd-move-up", { name: s.name })} disabled={i === 0} onClick={() => shift(i, i - 1)}>↑</button>
                <button type="button" className="os-btn os-btn-tool" aria-label={t("sgd-move-down", { name: s.name })} disabled={i === list.length - 1} onClick={() => shift(i, i + 1)}>↓</button>
                <button type="button" className="os-btn os-btn-tool" aria-label={t("sgd-delete", { name: s.name })} onClick={() => remove(s.id)}>×</button>
              </li>;
            })}</ul>
            <button type="button" className="os-btn os-btn-secondary" onClick={add}>{t("sgd-add")}</button>
            {preview.unassigned != null && <p className="os-hint" data-preview-unassigned>{t("sgd-unassigned", { count: f.integer(preview.unassigned) })}</p>}
            {list.length === 0 && <p className="os-hint">{t("sgd-empty")}</p>}
          </div>
          <div className="os-segdlg-editor">
            {current ? <>
              <label className="os-field"><span className="os-label">{t("sgd-name")}</span>
                <input className="os-input" aria-label={t("sgd-name")} value={current.name} aria-invalid={!!nameMessage || undefined} onChange={e => update(current.id, { name: e.target.value })} />
              </label>
              {nameMessage && <p className="os-error" role="alert" data-segdlg-error>{t(nameMessage.key, nameMessage.args)}</p>}
              <div role="group" aria-label={t("sgd-color")} className="os-segdlg-colors">
                {SWATCHES.map(c => <button type="button" key={c} className="os-swatch" style={{ background: c }} aria-label={c} aria-pressed={current.color.toLowerCase() === c.toLowerCase()} onClick={() => { update(current.id, { color: c }); setHex(null); }} />)}
                <input className="os-input" aria-label={t("sgd-color-custom")} value={colorValue} aria-invalid={!isHexColor(colorValue) || undefined} onChange={e => { setHex(e.target.value); if (isHexColor(e.target.value)) update(current.id, { color: e.target.value.toLowerCase() }); }} />
                <span className="os-segdlg-sample" style={{ background: isHexColor(current.color) ? current.color : undefined, color: isHexColor(current.color) ? segmentLabelColor(current.color) : undefined }}>{current.name || t("sgd-unnamed")}</span>
              </div>
              {!isHexColor(colorValue) && <p className="os-error" role="alert">{t("sgd-err-color")}</p>}
              <RuleEditor list={list} index={index} issues={validation.issuesOf(current) ?? []} columns={columns.names} pending={columns.pending} onChange={rules => update(current.id, { rules })} />
              {(validation.issuesOf(current) ?? []).filter(i => issueTarget(i.path).kind !== "condition").map((i, k) => <p key={k} className={i.warning ? "os-hint" : "os-error"} role={i.warning ? undefined : "alert"} data-segdlg-error={i.warning ? undefined : true}>{t(i.messageKey, i.args as Record<string, string>)}</p>)}
            </> : <p className="os-hint">{t("sgd-select")}</p>}
          </div>
        </div>
        <details className="os-segdlg-json">
          <summary>{t("sgd-json")}</summary>
          <textarea className="os-input" aria-label={t("sgd-json-text")} rows={6} value={json} onChange={e => setJson(e.target.value)} />
          <div className="os-segdlg-json-actions">
            <button type="button" className="os-btn os-btn-secondary" onClick={doExport}>{t("sgd-export")}</button>
            <button type="button" className="os-btn os-btn-secondary" onClick={applyImport} disabled={json.trim() === ""}>{t("sgd-import")}</button>
            <label className="os-btn os-btn-secondary">{t("sgd-import-file")}<input type="file" accept="application/json,.json" hidden onChange={e => void loadFile(e.target.files?.[0])} /></label>
          </div>
          {jsonError && <p className="os-error" role="alert">{t("sgd-err-json", { detail: jsonError })}</p>}
        </details>
      </>}
    </div>
  </Dialog>;
}
