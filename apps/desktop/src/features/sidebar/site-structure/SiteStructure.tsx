import { useEffect, useMemo, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useT, useFormat } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import type { SiteStructureView } from "../../../ipc/gen/SiteStructureView";
import type { PathNode } from "../../../ipc/gen/PathNode";
import { useSession } from "../../../state/session";
import { useSidebarNavigation } from "../frame/state";
import { QueryFeedback } from "../frame/QueryFeedback";
import { PanelActions } from "../frame/PanelActions";
import { Bars } from "./chart";
import { CLASS_COLOR, DEFAULT_COLOR, INDEXABILITY_CLASSES, TABLE_TARGET, classesOf, depthRows, flattenPaths, mergeChildren, NOTHING_LOADED, pathSearch, depthSearch, type LoadedChildren } from "./model";
import { navigateToTable } from "./navigate";

const VIEWS: SiteStructureView[] = ["indexability", "responseCodes", "crawlDepth"];
const PAGE = 100;
const VIEW_LABEL: Record<SiteStructureView, string> = { indexability: "ss-view-indexability", responseCodes: "ss-view-responseCodes", crawlDepth: "ss-view-crawlDepth" };

/** UI-066: directory tree (top) and depth distribution (bottom) with a view switch. */
export function SiteStructure() {
  const t = useT();
  const f = useFormat();
  const [view, setView] = useState<SiteStructureView>("indexability");
  const [toggled, setToggled] = useState<ReadonlySet<string>>(new Set());
  const [loaded, setLoaded] = useState<LoadedChildren>(NOTHING_LOADED);
  const [loadError, setLoadError] = useState<unknown>();
  const scope = useSidebarNavigation(s => s.scope);
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const version = useSession(s => s.sidebarVersion);
  const segment = scope.segment ?? undefined;
  const segmentKey = JSON.stringify(segment ?? null);
  const query = useQuery({
    queryKey: ["sidebar", "siteStructure", crawl, view, segmentKey, counts, version],
    queryFn: async ({ signal }) => ({ view, ...await sidebar.siteStructure({ view, ...(segment ? { segment } : {}), offset: 0, limit: PAGE }, signal) }),
    enabled: !!crawl,
    gcTime: 0,
    placeholderData: keepPreviousData,
  });
  // Lazily loaded pages belong to one crawl and one segment filter only.
  useEffect(() => { setLoaded(NOTHING_LOADED); setToggled(new Set()); setLoadError(undefined); }, [crawl, segmentKey]);

  async function loadMore(node: PathNode) {
    const have = mergeChildren(node, loaded).length;
    try {
      setLoadError(undefined);
      const page = await sidebar.siteStructure({ view, ...(segment ? { segment } : {}), path: node.path, offset: have, limit: PAGE });
      setLoaded(cur => ({
        children: { ...cur.children, [node.path]: [...(cur.children[node.path] ?? []), ...page.paths] },
        complete: page.paths.length < PAGE ? new Set(cur.complete).add(node.path) : cur.complete,
      }));
    } catch (e) { setLoadError(e); }
  }
  function toggle(node: PathNode, expanded: boolean) {
    setToggled(cur => { const next = new Set(cur); if (next.has(node.path)) next.delete(node.path); else next.add(node.path); return next; });
    if (!expanded && mergeChildren(node, loaded).length === 0) void loadMore(node);
  }

  const data = query.data;
  const rows = useMemo(() => data ? flattenPaths(data.paths, toggled, loaded) : [], [data, toggled, loaded]);
  // Depth rows, legend and bar clicks follow the view of the data on screen while a switch is still loading.
  const shown = data?.view ?? view;
  const depths = useMemo(() => data ? depthRows(data.depths, data.view) : [], [data]);
  const classes = classesOf(shown);
  const num = (n?: number) => n == null ? "–" : f.integer(n);
  const copyRows: (string | number)[][] = data ? [
    [t("ss-path"), t("sidebar-urls"), t("ss-indexable"), t("ss-non-indexable")],
    ...rows.map(r => [r.node.path, r.node.count, r.node.byClass[INDEXABILITY_CLASSES[0]] ?? 0, r.node.byClass[INDEXABILITY_CLASSES[1]] ?? 0]),
    [],
    [t("ss-depth"), t("ss-number-of-urls"), t("sidebar-percent-total"), ...classes],
    ...depths.map(d => [d.label, d.count, `${d.pct.toFixed(1)} %`, ...(shown === "crawlDepth" ? [] : d.parts.map(p => p.value))]),
  ] : [];

  return <div className="os-sidebar-site-structure">
    <QueryFeedback error={query.error ?? loadError} loading={!!crawl && query.isPending} retry={() => void query.refetch()} />
    {!crawl && <p>{t("sidebar-no-crawl")}</p>}
    {data && <>
      <PanelActions which="siteStructure" rows={copyRows} />
      <table className="os-sidebar-table" role="treegrid" aria-label={t("ss-directories")}>
        <thead><tr><th>{t("ss-path")}</th><th>{t("sidebar-urls")}</th><th>{t("ss-indexable")}</th><th>{t("ss-non-indexable")}</th></tr></thead>
        <tbody>{rows.flatMap(({ node, level, expanded, expandable, more }) => {
          const row = <tr key={node.path} aria-level={level} aria-expanded={expandable ? expanded : undefined}>
            <th scope="row" style={{ paddingInlineStart: 6 + (level - 1) * 12 }}><span className="os-sidebar-tree-name">
              {expandable
                ? <button type="button" className="os-sidebar-toggle" aria-label={`${t(expanded ? "ss-collapse" : "ss-expand")}: ${node.path}`} onClick={() => toggle(node, expanded)}>{expanded ? "▾" : "▸"}</button>
                : <span className="os-sidebar-toggle" aria-hidden="true" />}
              <button type="button" title={node.path} onClick={() => navigateToTable(TABLE_TARGET.tab, TABLE_TARGET.filter, pathSearch(node.path))}>{node.name || node.path}</button>
            </span></th>
            <td className="os-sidebar-count">{num(node.count)}</td><td className="os-sidebar-count">{num(node.byClass["Indexable"])}</td><td className="os-sidebar-count">{num(node.byClass["Non-Indexable"])}</td>
          </tr>;
          const extra = expanded && more && mergeChildren(node, loaded).length > 0
            ? [<tr key={`${node.path}\u0000more`} aria-level={level + 1}><td colSpan={4} style={{ paddingInlineStart: 6 + level * 12 }}><button type="button" onClick={() => void loadMore(node)}>{t("ss-load-more", { path: node.path })}</button></td></tr>]
            : [];
          return [row, ...extra];
        })}</tbody>
      </table>
      {rows.length === 0 && <p>{t("ss-no-urls")}</p>}
      <div className="os-sidebar-view"><label>{t("ss-view")} <select aria-label={t("ss-view")} value={view} onChange={e => setView(e.target.value as SiteStructureView)}>{VIEWS.map(v => <option key={v} value={v}>{t(VIEW_LABEL[v])}</option>)}</select></label></div>
      <table className="os-sidebar-table" aria-label={t("ss-depth-table")}>
        <thead><tr><th>{t("ss-depth")}</th><th>{t("ss-number-of-urls")}</th><th>{t("sidebar-percent-total")}</th></tr></thead>
        <tbody>{depths.map(d => <tr key={d.depth}>
          <th scope="row"><button type="button" onClick={() => navigateToTable(TABLE_TARGET.tab, TABLE_TARGET.filter, depthSearch(d.depth, shown))}>{d.label}</button></th>
          <td className="os-sidebar-count">{f.integer(d.count)}</td><td className="os-sidebar-count">{f.decimal(d.pct, 1)} %</td>
        </tr>)}</tbody>
      </table>
      <Bars label={t("ss-chart")} rows={depths.map(d => ({
        key: String(d.depth), label: d.label, total: d.count,
        parts: d.parts.map(p => ({ key: p.key, value: p.value, label: p.key === "all" ? t("ss-view-crawlDepth") : p.key, color: CLASS_COLOR[p.key] ?? DEFAULT_COLOR })),
      }))} onSelect={(row, part) => navigateToTable(TABLE_TARGET.tab, TABLE_TARGET.filter, depthSearch(Number(row), shown, part === "all" ? undefined : part))} />
      {classes.length > 0 && <ul className="os-sidebar-legend" aria-label={t("ss-legend")}>{classes.map(c => <li key={c}><i style={{ background: CLASS_COLOR[c] }} />{c}</li>)}</ul>}
      <p className="os-sidebar-note">{t("ss-note-depth")}</p>
    </>}
  </div>;
}
