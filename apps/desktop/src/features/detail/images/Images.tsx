import { useEffect, useState } from "react";
import { detail } from "../../../ipc/detail";
import { view } from "../../../ipc/view";
import { useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { Dialog } from "../../../ui/primitives/Dialog";
import { SearchField } from "../../../ui/primitives/fields";
import { Banner } from "../../../ui/primitives/Feedback";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import { useDetailRequest } from "../frame/request";
import { DetailError, DetailLoading } from "../frame/Feedback";
import type { DetailPanelProps } from "../frame/types";
export const IMAGE_COLUMNS = ["From", "To", "Alt Text", "Real Dimensions (WxH)", "Dimensions in Attributes (WxH)", "Display Dimensions (WxH)", "Potential Savings", "Path Type", "Link Path", "Link Position", "Link Origin", "Alt Text Length", "Source"];
export const IMAGE_HIDDEN = ["Alt Text Length", "Source"];

/** Resolves an instance's image URL without relying on detail row keys being URL ids. */
async function imageId(address: string, signal: AbortSignal): Promise<number | undefined> {
  const handle = await view.open({ spec: { kind: "tab", tab: "Images", filter: "All", sort: [], search: { kind: "rules", caseSensitive: true, tree: { kind: "cond", column: "Address", op: "equals", value: address } } }, columns: ["Address"] });
  try { const rows = await view.rows({ viewId: handle.viewId, version: handle.version, offset: 0, limit: 1, columns: ["Address"] }, signal); return rows.rowKeys[0]; }
  finally { await view.close({ viewId: handle.viewId }); }
}
export function ImagePreviewPanel({ urlId, address, crawlId, actions }: { urlId?: number; address?: string; crawlId?: string; actions: DetailPanelProps["actions"] }) {
  const t = useT(); const [consent, setConsent] = useState(false); const [zoom, setZoom] = useState(1); const [full, setFull] = useState(false);
  const key = JSON.stringify([urlId, address, crawlId]);
  const result = useDetailRequest(key, async signal => {
    const id = urlId ?? (address ? await imageId(address, signal) : undefined);
    if (id === undefined) return null;
    const [preview, fields] = await Promise.all([detail.imagePreview({ urlId: id }, signal), address ? Promise.resolve([]) : detail.urlDetails({ urlId: id }, signal)]);
    return { ...preview, address: address ?? String(fields.find(field => field.name === "Address")?.value ?? "") };
  });
  useEffect(() => { setConsent(false); setZoom(1); setFull(false); }, [key]);
  if (result.error) return <DetailError error={result.error} />;
  if (result.loading) return <DetailLoading />;
  const preview = result.value; if (!preview) return null;
  // An img element keeps SVG documents in image mode, with scripts disabled. Only the local protocol is accepted.
  if (!preview.url.startsWith("ospider://img/")) return <Banner kind="warning">{t("detail-image-unavailable")}</Banner>;
  const allowed = preview.stored || consent;
  const image = <img src={allowed ? preview.url : undefined} alt={preview.address || t("detail-image-preview")} style={{ width: preview.width * zoom, maxWidth: "none", height: "auto" }} />;
  return <section aria-label={t("detail-image-preview")} className="os-detail-preview">
    {!allowed ? <Button onPress={() => setConsent(true)}>{t("detail-image-fetch")}</Button> : <>
      <div><Button onPress={() => setZoom(z => Math.max(.1, z / 1.25))}>{t("detail-zoom-out")}</Button><Button onPress={() => setZoom(z => Math.min(8, z * 1.25))}>{t("detail-zoom-in")}</Button>
        <Button onPress={() => setFull(true)}>{t("detail-fullscreen")}</Button><span>{preview.width} × {preview.height}</span></div>
      <div className="os-detail-scroll">{image}</div>
      <Dialog title={t("detail-image-preview")} isOpen={full} onOpenChange={setFull} size="large"><div className="os-detail-scroll">{image}</div><Button onPress={() => setFull(false)}>{t("detail-close")}</Button></Dialog>
    </>}
    <Button onPress={() => { if (preview.address) actions.openBrowser(preview.address); }} isDisabled={!preview.address}>{t("detail-image-open")}</Button>
  </section>;
}
export function Images(props: DetailPanelProps) {
  const t = useT(); const [search, setSearch] = useState(""); const [sort, setSort] = useState<SortKey[]>([]); const [address, setAddress] = useState<string>();
  useEffect(() => { setAddress(undefined); }, [props.selection]);
  const focus = props.selection.focus ?? props.selection.urlIds[0];
  return <><SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} />
    {(props.selection.topTab === "Images" || address) && <ImagePreviewPanel key={`${focus}:${address}`} urlId={props.selection.topTab === "Images" && !address ? focus : undefined} address={address} crawlId={props.selection.crawlId} actions={props.actions} />}
    <TablePanel {...props} title={t("detail-tab-images")} columns={IMAGE_COLUMNS} hiddenColumns={IMAGE_HIDDEN}
      columnHints={!props.rendering ? { "Real Dimensions (WxH)": t("detail-js-only"), "Display Dimensions (WxH)": t("detail-js-only") } : {}}
      spec={{ kind: "images", urlIds: props.selection.urlIds, search, sort }} onSort={setSort} onSelectRow={get => setAddress(get("To"))} onActivateRow={get => setAddress(get("To"))} /></>;
}
