import { useMemo, useState, type ComponentType } from "react";
import { useT } from "../../../i18n";
import { Tabs } from "../../../ui/primitives/Tabs";
import { useSession } from "../../../state/session";
import { useOverview, useAnalysis } from "./useSidebarQuery";
import { useSidebarLayout } from "./layout";
import { Overview, overviewRows } from "../overview/Overview";
import { Issues } from "../issues/Issues";
import { Analysis, useStartAnalysis } from "../analysis/Analysis";
import { QueryFeedback } from "./QueryFeedback";
import { PanelActions } from "./PanelActions";
import { RegionBoundary } from "../../../app/ErrorBoundary";
import "./sidebar.css";

/** Future sidebar packages contribute panels without editing this frame. API panels ignore the scope store. */
export interface SidebarPanel { id: string; titleKey: string; Component: ComponentType; embeddingsOnly?: boolean }
const modules = import.meta.glob<{ panel?: SidebarPanel }>("../*/index.tsx", { eager: true });
const BASE = ["overview", "issues", "siteStructure", "segments", "responseTimes", "api", "spellingGrammar", "analysis"];

export function SidebarFrame({ panels = Object.values(modules).flatMap(m => m.panel ? [m.panel] : []), embeddingsEnabled = false }: { panels?: readonly SidebarPanel[]; embeddingsEnabled?: boolean }) {
  const t = useT();
  const { layout, setLayout, loaded, error } = useSidebarLayout();
  const [menu, setMenu] = useState<string | null>(null);
  const crawl = useSession(s => s.crawl?.crawlId);
  const overview = useOverview();
  const analysis = useAnalysis();
  const controls = useStartAnalysis();
  const ids = useMemo(() => [...new Set([...BASE, ...panels.filter(p => !p.embeddingsOnly || embeddingsEnabled).map(p => p.id)])], [panels, embeddingsEnabled]);
  const ordered = [...new Set([...layout.order.filter(id => ids.includes(id)), ...ids])];
  const shown = ordered.filter(id => !layout.hidden.includes(id));
  const selected = shown.includes(layout.selected) ? layout.selected : shown[0] ?? "overview";
  const title = (id: string) => t(panels.find(p => p.id === id)?.titleKey ?? `sidebar-tab-${id}`);
  const toggle = (id: string) => setLayout(l => ({ ...l, collapsed: l.collapsed.includes(id) ? l.collapsed.filter(x => x !== id) : [...l.collapsed, id] }));
  const needsAnalysis = !!analysis.data?.steps.some(s => s.stale || s.state === "pending") || !!overview.data?.tabs.some(tab => tab.filters.some(f => f.state.kind === "needsCrawlAnalysis"));
  const content = (id: string) => {
    if (id === "issues") return <Issues needsAnalysis={needsAnalysis} />;
    if (id === "analysis") return <Analysis controls={controls} />;
    if (id === "overview") return <><QueryFeedback error={overview.error} loading={!!crawl && overview.isPending} retry={() => void overview.refetch()} />{!crawl && <p>{t("sidebar-no-crawl")}</p>}{overview.data && <><PanelActions which="overview" rows={overviewRows(overview.data)} /><Overview data={overview.data} staleSteps={analysis.data?.steps.filter(s => s.stale).map(s => s.step) ?? []} collapsed={layout.collapsed} toggle={toggle} startAnalysis={() => void controls.start()} /></>}</>;
    const Panel = panels.find(p => p.id === id)?.Component;
    return Panel ? <Panel /> : <p>{t("sidebar-panel-unavailable")}</p>;
  };
  return <div className="os-sidebar" data-sidebar-ready={loaded}>
    <QueryFeedback error={error ?? controls.error} />
    {analysis.data?.steps.some(s => s.stale) && selected !== "analysis" && <p role="status">{t("sidebar-analysis-stale")} <button type="button" disabled={analysis.data.running || controls.starting} onClick={() => void controls.start()}>{t("sidebar-reanalyze")}</button></p>}
    <Tabs label={t("sidebar-tabs")} selected={selected} onSelect={id => setLayout(l => ({ ...l, selected: id }))} tabs={shown.map(id => ({ id, label: <span draggable onDragStart={e => e.dataTransfer.setData("text/plain", id)} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); const from = e.dataTransfer.getData("text/plain"); if (from === id || !ordered.includes(from)) return; const order = ordered.filter(x => x !== from); order.splice(order.indexOf(id), 0, from); setLayout(l => ({ ...l, order })); }} onContextMenu={e => { e.preventDefault(); setMenu(id); }}>{title(id)}</span>, content: <RegionBoundary name={`sidebar-${id}`}>{content(id)}</RegionBoundary> }))} />
    {menu && <div role="group" aria-label={t("sidebar-customize")}><button type="button" disabled={shown.length <= 1} onClick={() => { setLayout(l => ({ ...l, hidden: [...l.hidden, menu] })); setMenu(null); }}>{t("sidebar-hide")} {title(menu)}</button><button type="button" onClick={() => setMenu(null)}>{t("sidebar-cancel")}</button></div>}
    <details><summary>{t("sidebar-customize")}</summary>{ordered.map(id => <label key={id}><input type="checkbox" checked={!layout.hidden.includes(id)} disabled={shown.length === 1 && shown.includes(id)} onChange={() => setLayout(l => ({ ...l, hidden: l.hidden.includes(id) ? l.hidden.filter(x => x !== id) : [...l.hidden, id] }))} />{title(id)}<button type="button" aria-label={`${t("sidebar-move-first")}: ${title(id)}`} onClick={() => setLayout(l => ({ ...l, order: [id, ...ordered.filter(x => x !== id)] }))}>↑</button></label>)}</details>
  </div>;
}
