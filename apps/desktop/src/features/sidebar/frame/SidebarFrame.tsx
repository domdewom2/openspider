import { ComparisonSidebar } from "../../compare/ComparisonSidebar";
import { useComparison } from "../../compare/model";
import { useCallback, useDeferredValue, useMemo, useState, type ComponentType } from "react";
import { useT } from "../../../i18n";
import { TabBar } from "../../../ui/primitives/TabBar";
import { TabContextMenuHost, TabsConfigDialog, tabContextEntries } from "../../table-advanced/TabControls";
import { useSession } from "../../../state/session";
import { useOverview, useAnalysis } from "./useSidebarQuery";
import { useSidebarLayout } from "./layout";
import { Overview, overviewRows } from "../overview/Overview";
import { Issues } from "../issues/Issues";
import { AnalysisNotice, useStartAnalysis } from "../analysis/AnalysisNotice";
import { QueryFeedback } from "./QueryFeedback";
import { PanelActions } from "./PanelActions";
import { RegionBoundary } from "../../../app/ErrorBoundary";
import "../../shell/tabbar.css";
import "./sidebar.css";

/** Future sidebar packages contribute panels without editing this frame. API panels ignore the scope store. */
export interface SidebarPanel { id: string; titleKey: string; Component: ComponentType; embeddingsOnly?: boolean }
const modules = import.meta.glob<{ panel?: SidebarPanel }>("../*/index.tsx", { eager: true });
const MANDATORY = "overview";
const BASE = ["overview", "issues", "siteStructure", "segments", "responseTimes", "api", "spellingGrammar"];

export function SidebarFrame({ panels = Object.values(modules).flatMap(m => m.panel ? [m.panel] : []), embeddingsEnabled = false }: { panels?: readonly SidebarPanel[]; embeddingsEnabled?: boolean }) {
  const activeComparison = useComparison(s => !!s.session);
  const t = useT();
  const { layout, setLayout, loaded, error } = useSidebarLayout();
  const [configure, setConfigure] = useState(false);
  const crawl = useSession(s => s.crawl?.crawlId);
  const overview = useOverview();
  // Keep header control acknowledgements ahead of the large filter overview.
  const overviewData = useDeferredValue(overview.data);
  const analysis = useAnalysis();
  const controls = useStartAnalysis();
  const ids = useMemo(() => [...new Set([...BASE, ...panels.filter(p => !p.embeddingsOnly || embeddingsEnabled).map(p => p.id)])], [panels, embeddingsEnabled]);
  const ordered = [...new Set([...layout.order.filter(id => ids.includes(id)), ...ids])];
  const hiddenTabs = layout.hidden.filter(id => id !== MANDATORY && ordered.includes(id));
  const shown = ordered.filter(id => !hiddenTabs.includes(id));
  const selected = shown.includes(layout.selected) ? layout.selected : shown[0] ?? "overview";
  const title = (id: string) => t(panels.find(p => p.id === id)?.titleKey ?? `sidebar-tab-${id}`);
  const tabLayout = { order: ordered, hidden: hiddenTabs, mandatory: MANDATORY };
  const hide = (ids: string[]) => setLayout(l => ({ ...l, hidden: [...new Set([...l.hidden, ...ids.filter(id => id !== MANDATORY)])] }));
  const reset = () => setLayout(l => ({ ...l, order: [], hidden: [], selected: MANDATORY }));
  const toggle = useCallback((id: string) => setLayout(l => ({ ...l, collapsed: l.collapsed.includes(id) ? l.collapsed.filter(x => x !== id) : [...l.collapsed, id] })), [setLayout]);
  const staleSteps = useMemo(() => analysis.data?.steps.filter(s => s.stale).map(s => s.step) ?? [], [analysis.data?.steps]);
  const startAnalysis = useCallback(() => void controls.start(), [controls.start]);
  const needsAnalysis = !!analysis.data?.steps.some(s => s.stale || s.state === "pending") || !!overview.data?.tabs.some(tab => tab.filters.some(f => f.state.kind === "needsCrawlAnalysis"));
  const content = (id: string) => {
    if (id === "issues") return <Issues needsAnalysis={needsAnalysis} />;
    if (id === "overview") return <><QueryFeedback error={overview.error} loading={!!crawl && overview.isPending} retry={() => void overview.refetch()} />{!crawl && <p>{t("sidebar-no-crawl")}</p>}{overviewData && <><PanelActions which="overview" rows={overviewRows(overviewData, t)} /><Overview data={overviewData} staleSteps={staleSteps} collapsed={layout.collapsed} toggle={toggle} startAnalysis={startAnalysis} /></>}</>;
    const Panel = panels.find(p => p.id === id)?.Component;
    return Panel ? <Panel /> : <p>{t("sidebar-panel-unavailable")}</p>;
  };
  if (activeComparison) return <ComparisonSidebar />;
  return <div className="os-sidebar" data-sidebar-ready={loaded}>
    <QueryFeedback error={error ?? controls.error ?? analysis.error} />
    <AnalysisNotice status={analysis.data} controls={controls} />
    {/* UI-060/UI-112, A02-03/A02-06: one-line tabs with the shared overflow menu and tab context menu; Overview is the mandatory tab. */}
    <TabContextMenuHost className="os-sidebar-tabs" label={t("u20-tab-menu")} entriesFor={id => tabContextEntries(t, id, tabLayout, { hide, configure: () => setConfigure(true), reset })}>
      <TabBar tabs={shown.map(id => ({ id, label: title(id) }))} selected={selected} onSelect={id => setLayout(l => ({ ...l, selected: id }))}
        label={t("sidebar-tabs")} overflowLabel={t("sidebar-more-tabs")}
        onReorder={ids => setLayout(l => ({ ...l, order: [...ids, ...ordered.filter(id => !ids.includes(id))] }))}
        config={{ all: ordered.map(id => ({ id, label: title(id) })), hidden: hiddenTabs, mandatory: MANDATORY,
          mandatoryReason: t("u20-tab-mandatory"), configureLabel: t("u20-tabs-configure"), resetLabel: t("u20-tabs-reset"),
          onToggle: (id, visible) => visible ? setLayout(l => ({ ...l, hidden: l.hidden.filter(x => x !== id) })) : hide([id]),
          onConfigure: () => setConfigure(true), onReset: reset }} />
    </TabContextMenuHost>
    <div className="os-sidebar-panel" role="tabpanel" aria-label={title(selected)}>
      <RegionBoundary key={selected} name={`sidebar-${selected}`}>{content(selected)}</RegionBoundary>
    </div>
    <TabsConfigDialog isOpen={configure} onOpenChange={setConfigure} layout={tabLayout} label={title}
      onApply={(order, hidden) => setLayout(l => ({ ...l, order, hidden }))} />
  </div>;
}
