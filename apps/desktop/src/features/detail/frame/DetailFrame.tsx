import { ComparisonDetail } from "../../compare/ComparisonDetail";
import { useComparison } from "../../compare/model";
import { useEffect, useMemo, useRef, useState } from "react";
import { RegionBoundary } from "../../../app/ErrorBoundary";
import { config } from "../../../ipc/config";
import { eventBus } from "../../../ipc/events";
import { exportApi } from "../../../ipc/export";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";
import type { Selection } from "../../../ipc/gen/Selection";
import type { ExportFormatDto } from "../../../ipc/gen/ExportFormatDto";
import { useSession } from "../../../state/session";
import { useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { Menu } from "../../../ui/primitives/Menu";
import { ContextMenu } from "../../../ui/primitives/ContextMenu";
import { TabBar } from "../../../ui/primitives/TabBar";
import { Dialog } from "../../../ui/primitives/Dialog";
import { TextField, Select } from "../../../ui/primitives/fields";
import { Banner } from "../../../ui/primitives/Feedback";
import { UrlDetails } from "../url-details/UrlDetails";
import { Hreflang } from "../url-details/Hreflang";
import { Inlinks } from "../inlinks/Links";
import { Outlinks } from "../outlinks/Outlinks";
import { Images } from "../images/Images";
import { Resources } from "../resources/Resources";
import { Headers } from "../headers/Headers";
import { Cookies } from "../cookies/Cookies";
import { CrawlPath } from "../crawl-path/CrawlPath";
import { DetailEmpty, DetailError } from "./Feedback";
import { useDetailPreferences } from "./preferences";
import { DETAIL_BACK_EVENT, DETAIL_SHOW_TAB_EVENT, requestNavigation, useDetailSelection } from "./selection";
import { requestBrowser } from "./browser";
import { DETAIL_TABS, type DetailActions, type DetailPanelProps, type DetailPlugin, type DetailSelection, type DetailTabId } from "./types";
import "./detail.css";

const extensionModules = import.meta.glob<{ detailTab?: DetailPlugin }>("../*/index.tsx", { eager: true });
const PANELS: Partial<Record<DetailTabId, DetailPlugin["Component"]>> = { urlDetails: UrlDetails, inlinks: Inlinks, outlinks: Outlinks, images: Images, resources: Resources, headers: Headers, cookies: Cookies };
const UNION_TABS = new Set<DetailTabId>(["inlinks", "outlinks", "images", "resources", "nGrams"]);
const DEFAULT_POSITIONS = ["Head", "Header", "Nav", "Content", "Sidebar", "Footer"];
export interface DetailFrameProps {
  selection?: DetailSelection;
  actions?: Partial<DetailActions>;
  rendering?: boolean;
  htmlStored?: boolean;
  storeUncrawlable?: boolean;
  positions?: readonly string[];
  listMode?: boolean;
  plugins?: DetailPlugin[];
}
export function DetailFrame(props: DetailFrameProps) {
  const activeComparison = useComparison(s => !!s.session);
  return activeComparison ? <ComparisonDetail /> : <RegularDetailFrame {...props} />;
}
function RegularDetailFrame(props: DetailFrameProps) {
  const t = useT(); const shared = useDetailSelection(s => s.selection); const selection = props.selection ?? shared;
  const crawl = useSession(s => s.crawl);
  const [active, setActive] = useState<DetailTabId>("urlDetails"); const [error, setError] = useState<unknown>();
  const [showCrawlPath, setShowCrawlPath] = useState(false);
  const [features, setFeatures] = useState({ rendering: false, htmlStored: false, storeUncrawlable: false, positions: DEFAULT_POSITIONS });
  const [preferences, savePreferences, loaded] = useDetailPreferences(setError);
  const preferencesRef = useRef(preferences); preferencesRef.current = preferences;
  const savePreferencesRef = useRef(savePreferences); savePreferencesRef.current = savePreferences;
  const [exporting, setExporting] = useState<{ spec: ViewSpec; selection: Selection }>();
  const [path, setPath] = useState(""); const [format, setFormat] = useState<ExportFormatDto>("csv"); const [exportBusy, setExportBusy] = useState(false);
  const [exportOp, setExportOp] = useState<string>(); const op = useSession(s => exportOp ? s.ops[exportOp] : undefined);
  const [configVersion, setConfigVersion] = useState(0);
  // M2-U21 (additiv): Fach-Tabs können einen anderen Detail-Tab anfordern (z. B. Accessibility → View Source); ein ausgeblendeter Tab wird wieder eingeblendet.
  useEffect(() => {
    const show = (event: Event) => {
      const tab = (event as CustomEvent<{ tab?: string }>).detail?.tab as DetailTabId | undefined;
      if (!tab || !DETAIL_TABS.includes(tab)) return;
      setActive(tab); setShowCrawlPath(false);
      if (preferencesRef.current.hidden.includes(tab)) savePreferencesRef.current({ ...preferencesRef.current, hidden: preferencesRef.current.hidden.filter(id => id !== tab) });
    };
    document.addEventListener(DETAIL_SHOW_TAB_EVENT, show);
    return () => document.removeEventListener(DETAIL_SHOW_TAB_EVENT, show);
  }, []);
  useEffect(() => eventBus.on("settingsChanged", () => setConfigVersion(version => version + 1)), []);
  useEffect(() => {
    const controller = new AbortController();
    void config.get(controller.signal).then(dto => {
      const cfg = dto.config as { rendering?: { mode?: string }; extraction?: { store_html?: boolean }; spider?: { store_uncrawlable_links?: boolean }; link_positions?: { rules?: { position: string }[] } };
      if (!controller.signal.aborted) setFeatures({ rendering: cfg.rendering?.mode === "javascript", htmlStored: cfg.extraction?.store_html === true, storeUncrawlable: cfg.spider?.store_uncrawlable_links === true,
        positions: [...new Set([...DEFAULT_POSITIONS, ...(cfg.link_positions?.rules ?? []).map(p => p.position)])] });
    }).catch(() => undefined);
    return () => controller.abort();
  }, [selection.crawlId, crawl?.crawlId, configVersion]);
  const actions: DetailActions = useMemo(() => ({
    navigate: requestNavigation,
    back: () => { document.dispatchEvent(new CustomEvent(DETAIL_BACK_EVENT)); },
    openBrowser: requestBrowser,
    export: (spec, selection) => { setExporting({ spec, selection }); setPath(""); },
    reportError: setError,
    ...props.actions,
  }), [props.actions]);
  const panels = { ...PANELS };
  for (const plugin of [...Object.values(extensionModules).flatMap(m => m.detailTab ? [m.detailTab] : []), ...(props.plugins ?? [])]) panels[plugin.id] = plugin.Component;
  if (selection.topTab === "Hreflang") panels.urlDetails = Hreflang;
  const tab = preferences.hidden.includes(active) ? "urlDetails" : active;
  const Panel = panels[tab];
  const effectiveSelection = { ...selection, crawlId: selection.crawlId ?? crawl?.crawlId };
  const panelProps: DetailPanelProps = { selection: effectiveSelection, actions, rendering: props.rendering ?? features.rendering,
    htmlStored: props.htmlStored ?? features.htmlStored, storeUncrawlable: props.storeUncrawlable ?? features.storeUncrawlable, positions: props.positions ?? features.positions, listMode: props.listMode ?? crawl?.mode === "list" };
  const tabs = preferences.order.filter(id => !preferences.hidden.includes(id)).map(id => ({ id, label: t(`detail-tab-${id}`) }));
  const selected = selection.urlIds.length > 0 && selection.related !== false;
  const multiFocus = selection.urlIds.length > 1 && !UNION_TABS.has(tab) && !(tab === "urlDetails" && selection.topTab === "Hreflang");
  return <section className="os-detail" role="region" aria-label={t("detail-region")} onKeyDown={event => {
    if (event.altKey && event.key === "ArrowLeft") { event.preventDefault(); actions.back(); }
  }}>
    <div className="os-detail-heading"><h2>{t("detail-region")}</h2>
      <Menu label={t("detail-configure-tabs")} trigger={<Button>{t("detail-configure-tabs")}</Button>} entries={preferences.order.map(id => ({ id, label: `${preferences.hidden.includes(id) ? "" : "✓ "}${t(`detail-tab-${id}`)}`,
        isDisabled: id === "urlDetails" || !loaded, onAction: () => savePreferences({ ...preferences, hidden: preferences.hidden.includes(id) ? preferences.hidden.filter(t => t !== id) : [...preferences.hidden, id] }),
      }))} />
      <Button isDisabled={!loaded} onPress={() => { savePreferences({ order: [], hidden: [] }); setActive("urlDetails"); }}>{t("detail-reset-tabs")}</Button>
      <Button isDisabled={!selected || !loaded} onPress={() => setShowCrawlPath(true)}>{t("detail-tab-crawlPath")}</Button>
    </div>
    <ContextMenu label={t("detail-configure-tabs")} entries={[
      { id: "close", label: t("detail-close"), isDisabled: tab === "urlDetails" || !loaded, onAction: () => savePreferences({ ...preferences, hidden: [...preferences.hidden, tab] }) },
      { id: "others", label: t("detail-close-others"), isDisabled: !loaded, onAction: () => savePreferences({ ...preferences, hidden: preferences.order.filter(id => id !== tab && id !== "urlDetails") }) },
      { id: "reset", label: t("detail-reset-tabs"), isDisabled: !loaded, onAction: () => savePreferences({ order: [], hidden: [] }) },
    ]}>
      <TabBar tabs={tabs} selected={tab} onSelect={id => setActive(id as DetailTabId)} label={t("detail-tabs")} overflowLabel={t("detail-more-tabs")}
        onReorder={ids => { if (loaded) savePreferences({ ...preferences, order: [...ids, ...preferences.hidden] as DetailTabId[] }); }} />
    </ContextMenu>
    {error ? <DetailError error={error} /> : null}
    {op && <Banner kind={op.error ? "error" : "info"}>{op.error ? t(`err-code-${op.error.code.toLowerCase()}`) : t("detail-export-status", { status: op.state })}</Banner>}
    <div className="os-detail-body" role="tabpanel" aria-label={t(`detail-tab-${tab}`)}>
      {!selected ? <DetailEmpty state={{ code: "NO_SELECTION" }} /> : showCrawlPath ? null : <>
        {multiFocus && <Banner>{t("detail-focus", { id: selection.focus ?? selection.urlIds[0] })}</Banner>}
        {Panel ? <RegionBoundary key={`${tab}:${selection.crawlId ?? crawl?.crawlId ?? ""}`} name="detail"><Panel {...panelProps} /></RegionBoundary> : <DetailEmpty state={{ code: "NO_DATA_YET" }} />}
      </>}
    </div>
    <Dialog title={t("detail-tab-crawlPath")} isOpen={showCrawlPath} onOpenChange={setShowCrawlPath} size="large"
      footer={close => <Button onPress={close}>{t("detail-close")}</Button>}>
      {selected ? <RegionBoundary key={effectiveSelection.crawlId} name="detail"><CrawlPath {...panelProps} /></RegionBoundary> : <DetailEmpty state={{ code: "NO_SELECTION" }} />}
    </Dialog>
    <Dialog title={t("detail-export")} isOpen={!!exporting} onOpenChange={open => { if (!open && !exportBusy) setExporting(undefined); }}>
      <TextField label={t("detail-export-path")} value={path} onChange={setPath} />
      <Select label={t("detail-export-format")} value={format} onChange={value => setFormat(value as ExportFormatDto)} options={[{ value: "csv", label: "CSV" }, { value: "xlsx", label: "XLSX" }]} />
      <Button isDisabled={!path.trim() || exportBusy} onPress={() => {
        if (!exporting) return; setExportBusy(true);
        void exportApi.detail({ ...exporting, path, format }).then(handle => { setExportOp(handle.opId); setExporting(undefined); }).catch(setError).finally(() => setExportBusy(false));
      }}>{t("detail-export")}</Button>
    </Dialog>
  </section>;
}
