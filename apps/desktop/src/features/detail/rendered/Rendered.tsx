import { useEffect, useRef, useState } from "react";
import { detail } from "../../../ipc/detail";
import type { ScreenshotInfo } from "../../../ipc/gen/ScreenshotInfo";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { ContextMenu } from "../../../ui/primitives/ContextMenu";
import { RadioGroup, Select } from "../../../ui/primitives/fields";
import { DetailError, DetailLoading } from "../frame/Feedback";
import { useDetailRequest } from "../frame/request";
import { TablePanel } from "../frame/TablePanel";
import type { DetailPanelProps } from "../frame/types";
import "./rendered.css";
import { FachEmpty, featureDisabled, fileStem, focusOf, gate, needsRendering, saveFile, useFachConfig } from "./common";

export const RESOURCE_COLUMNS = ["Address", "Type", "Status Code", "Blocked By"] as const;
/** Kachel `n` von `tiles` hat die Höhe `ceil(height / tiles)` (letzte Kachel der Rest); Basis der Platzhalter vor dem Laden. */
export const tileHeight = (info: Pick<ScreenshotInfo, "height" | "tiles">): number => Math.max(1, Math.ceil(info.height / Math.max(1, info.tiles)));

function Tile({ src, index, total, width, height, scale, label }: { src: string; index: number; total: number; width: number; height: number; scale: number; label: string }) {
  const host = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(typeof IntersectionObserver === "undefined");
  const [natural, setNatural] = useState<number>();
  useEffect(() => {
    if (near || !host.current) return;
    const observer = new IntersectionObserver(entries => { if (entries.some(e => e.isIntersecting)) { setNear(true); observer.disconnect(); } }, { rootMargin: "600px 0px" });
    observer.observe(host.current);
    return () => observer.disconnect();
  }, [near]);
  const shownHeight = (natural ?? height) * scale;
  return <div ref={host} className="os-rendered-tile" style={{ width: width * scale, height: shownHeight }} data-tile={index}>
    {near && <img src={src} alt={label} loading="lazy" decoding="async" draggable={false} style={{ width: width * scale, height: "auto", display: "block" }}
      onLoad={event => { const img = event.currentTarget; if (img.naturalWidth > 0) setNatural(img.naturalHeight * (width / img.naturalWidth)); }} />}
    <span className="os-visually-hidden">{index + 1}/{total}</span>
  </div>;
}

export function Rendered(props: DetailPanelProps) {
  const t = useT(); const cfg = useFachConfig(props.selection.crawlId);
  const { actions } = props; const focus = focusOf(props.selection);
  const [zoom, setZoom] = useState<"fit" | "100">("fit"); const [filter, setFilter] = useState<"blocked" | "all">("blocked");
  const [sort, setSort] = useState<SortKey[]>([]); const [width, setWidth] = useState(0);
  const area = useRef<HTMLDivElement>(null);
  const empty = cfg.loaded && gate(!(props.rendering || cfg.rendering) && needsRendering(), (props.rendering || cfg.rendering) && !cfg.screenshots && featureDisabled("rendering.screenshots.enabled"));
  const ready = cfg.loaded && !empty;
  const shot = useDetailRequest(`shot:${props.selection.crawlId}:${focus}:${ready}`, signal => ready ? detail.screenshot({ urlId: focus }, signal) : Promise.resolve(undefined));
  useEffect(() => {
    const node = area.current; if (!node) return;
    const measure = () => setWidth(node.clientWidth);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure); observer.observe(node);
    return () => observer.disconnect();
  }, [ready, shot.value]);
  if (!cfg.loaded) return <DetailLoading />;
  if (empty) return <div className="os-fach"><FachEmpty state={empty} onError={actions.reportError} /></div>;
  if (shot.error) return <DetailError error={shot.error} />;
  if (shot.loading) return <DetailLoading />;
  const info = shot.value;
  const scale = !info ? 1 : zoom === "100" || width <= 0 ? 1 : Math.min(1, Math.max(0.05, (width - 24) / info.width));
  const save = async () => {
    if (!info) return;
    try {
      const blobs = await Promise.all(Array.from({ length: info.tiles }, async (_, tile) => (await fetch(`${info.url}/${tile}`)).blob()));
      const bitmaps = await Promise.all(blobs.map(blob => createImageBitmap(blob)));
      const canvas = document.createElement("canvas"); canvas.width = info.width; canvas.height = bitmaps.reduce((sum, b) => sum + b.height, 0);
      const context = canvas.getContext("2d"); if (!context) throw new Error("canvas");
      let y = 0; for (const bitmap of bitmaps) { context.drawImage(bitmap, 0, y); y += bitmap.height; }
      const png = await new Promise<Blob>((resolve, reject) => canvas.toBlob(b => b ? resolve(b) : reject(new Error("png")), "image/png"));
      const address = String((await detail.urlDetails({ urlId: focus })).find(row => row.name === "Address")?.value ?? focus);
      saveFile(`${fileStem(address)}.png`, png, "image/png");
    } catch (error) { actions.reportError(error); }
  };
  return <div className="os-fach os-rendered">
    <div className="os-fach-toolbar">
      <RadioGroup label={t("rendered-zoom")} orientation="horizontal" value={zoom} onChange={value => setZoom(value as "fit" | "100")} options={[{ value: "fit", label: t("rendered-zoom-fit") }, { value: "100", label: "100 %" }]} />
      <Button onPress={() => void save()} isDisabled={!info || info.tiles === 0}>{t("rendered-save")}</Button>
    </div>
    <div className="os-fach-split os-fach-split-stack">
      <section className="os-fach-pane" aria-label={t("rendered-screenshot")}>
        <div className="os-fach-pane-body os-rendered-shot" ref={area} tabIndex={0}>
          {!info || info.tiles === 0 ? <FachEmpty state={{ code: "NO_DATA_YET" }} /> :
            <ContextMenu label={t("rendered-screenshot")} entries={[{ id: "save", label: t("rendered-save"), onAction: () => void save() }]}>
              <figure className="os-rendered-figure" aria-label={t("rendered-screenshot-size", { width: info.width, height: info.height })} data-tiles={info.tiles}>
                {Array.from({ length: info.tiles }, (_, tile) => <Tile key={tile} src={`${info.url}/${tile}`} index={tile} total={info.tiles} width={info.width} height={tileHeight(info)} scale={scale}
                  label={t("rendered-tile", { n: tile + 1, total: info.tiles })} />)}
              </figure>
            </ContextMenu>}
        </div>
      </section>
      <section className="os-fach-pane" aria-label={t("rendered-resources")}>
        <div className="os-fach-toolbar"><Select label={t("rendered-filter")} value={filter} onChange={value => setFilter(value as "blocked" | "all")}
          options={[{ value: "blocked", label: t("rendered-filter-blocked") }, { value: "all", label: t("rendered-filter-all") }]} /></div>
        <div className="os-fach-pane-body os-fach-fill">
          <TablePanel {...props} title={t("rendered-resources")} columns={RESOURCE_COLUMNS} plainContext
            spec={{ kind: "detail", detail: filter === "blocked" ? "blockedResources" : "network", urlIds: [focus], params: {}, sort }} onSort={setSort} />
        </div>
      </section>
    </div>
  </div>;
}
