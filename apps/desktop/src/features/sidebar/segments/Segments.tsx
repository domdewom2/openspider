import { Button } from "../../../ui/primitives/Button";
import { useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useT, useFormat } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import type { SegmentRow } from "../../../ipc/gen/SegmentRow";
import { useSession } from "../../../state/session";
import { segmentLabelColor } from "../../../ui/tokens/tokens";
import { useSidebarNavigation } from "../frame/state";
import { QueryFeedback } from "../frame/QueryFeedback";
import { PanelActions } from "../frame/PanelActions";
import { openSegmentsDialog } from "../../segments-dialog/store";
import { SEGMENT_VIEWS, classValue, columnsOf, coreView, hasSegments, orderRows, segmented, type SegmentsView } from "./model";

/** Label in the segment colour with automatically black or white text (UI-020, ≥ 4.5:1). */
export function SegmentLabel({ name, color }: { name: string; color?: string | null }) {
  return color
    ? <span className="os-sidebar-segment-label" style={{ background: color, color: segmentLabelColor(color) }}>{name}</span>
    : <span className="os-sidebar-segment-label os-sidebar-segment-none">{name}</span>;
}

/** UI-067: key figures per segment; a click sets the global segment filter (UI-020). */
export function Segments() {
  const t = useT();
  const f = useFormat();
  const [view, setView] = useState<SegmentsView>("overview");
  const scope = useSidebarNavigation(s => s.scope);
  const setScope = useSidebarNavigation(s => s.setScope);
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const issues = useSession(s => s.issuesVersion);
  const version = useSession(s => s.sidebarVersion);
  const query = useQuery({
    queryKey: ["sidebar", "segments", crawl, view, counts, issues, version],
    // The columns follow the view of the data on screen, not the selector, so a switch never shows empty cells.
    queryFn: async ({ signal }) => ({ view, rows: await sidebar.segments({ view: coreView(view) }, signal) }),
    enabled: !!crawl,
    gcTime: 0,
    placeholderData: keepPreviousData,
  });
  const rows = query.data ? orderRows(query.data.rows) : undefined;
  const columns = columnsOf(query.data?.view ?? view);
  const share = rows ? segmented(rows) : undefined;
  const label = (row: SegmentRow) => row.name ?? t("sg-none");
  const active = scope.segment?.kind === "segment" ? scope.segment.name : scope.segment?.kind === "none" ? null : undefined;
  const select = (row: SegmentRow) => setScope({ ...scope, segment: row.name == null ? { kind: "none" } : { kind: "segment", name: row.name } });
  const clear = () => { const { segment: _unused, ...rest } = scope; setScope(rest); };
  const cell = (v: number | null) => v == null ? "–" : f.integer(v);

  return <div className="os-sidebar-segments-panel">
    <div className="os-sidebar-actions">
      <Button onPress={() => openSegmentsDialog()}>{t("sg-configure")}</Button>
      {scope.segment && <Button onPress={clear}>{t("sg-clear-filter")}</Button>}
    </div>
    <QueryFeedback error={query.error} loading={!!crawl && query.isPending} retry={() => void query.refetch()} />
    {!crawl && <p>{t("sidebar-no-crawl")}</p>}
    {rows && share && !hasSegments(rows) && <div className="os-sidebar-empty" data-empty-state="no-segments"><p>{t("sg-empty")}</p><Button onPress={() => openSegmentsDialog()}>{t("sg-create")}</Button></div>}
    {rows && share && hasSegments(rows) && <>
      <PanelActions which="segments" rows={[[t("sg-segment"), t("sidebar-urls"), t("sg-percent-segmented"), ...columns.map(c => c.header)], ...rows.map(r => [label(r), r.urls, `${share.pct(r).toFixed(1)} %`, ...columns.map(c => classValue(r, c.key) ?? "–")])]} />
      <div className="os-sidebar-view"><label>{t("sg-view")} <select aria-label={t("sg-view")} value={view} onChange={e => setView(e.target.value as SegmentsView)}>{SEGMENT_VIEWS.map(v => <option key={v} value={v}>{t(`sg-view-${v}`)}</option>)}</select></label></div>
      <table className="os-sidebar-table" aria-label={t("sg-table")}>
        <thead><tr><th>{t("sg-segment")}</th><th>{t("sidebar-urls")}</th><th>{t("sg-percent-segmented")}</th>{columns.map(c => <th key={c.key}>{c.header}</th>)}</tr></thead>
        <tbody>{rows.map(r => <tr key={r.name ?? "\u0000none"} aria-selected={active === undefined ? undefined : active === r.name}>
          <th scope="row"><button type="button" onClick={() => select(r)} aria-label={t("sg-filter-by", { name: label(r) })}><SegmentLabel name={label(r)} color={r.color} /></button></th>
          <td className="os-sidebar-count">{f.integer(r.urls)}</td><td className="os-sidebar-count">{f.decimal(share.pct(r), 1)} %</td>
          {columns.map(c => <td key={c.key} className="os-sidebar-count">{cell(classValue(r, c.key))}</td>)}
        </tr>)}</tbody>
      </table>
    </>}
  </div>;
}
