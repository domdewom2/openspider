import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button as RacButton, Popover, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import { ChevronRight, CircleAlert, TriangleAlert } from "lucide-react";
import { detail } from "../../../ipc/detail";
import type { SdIssueDto } from "../../../ipc/gen/SdIssueDto";
import { useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { MenuBody } from "../../../ui/primitives/Menu";
import { DetailError, DetailLoading } from "../frame/Feedback";
import { useDetailRequest } from "../frame/request";
import type { DetailPanelProps } from "../frame/types";
import { copyText, FachEmpty, featureDisabled, focusOf, gate, useFachConfig } from "../rendered/common";
import { buildTree, flatten, ownerOf, severityOf, summary, toJsonLd, validationType, type Severity, type TreeEntry } from "./model";
import "./sd.css";

function Symbol({ severity, label }: { severity: Severity; label: string }) {
  if (severity === "none") return null;
  const Icon = severity === "error" ? CircleAlert : TriangleAlert;
  return <span className={`os-sd-symbol os-sd-${severity}`} role="img" aria-label={label} title={label}><Icon size={13} aria-hidden /></span>;
}

export function StructuredData(props: DetailPanelProps) {
  const t = useT(); const cfg = useFachConfig(props.selection.crawlId); const { actions } = props; const focus = focusOf(props.selection);
  const [selectedId, setSelectedId] = useState<string>();
  const [menu, setMenu] = useState<{ id: string; x: number; y: number }>();
  const anchorRef = useRef<HTMLSpanElement>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const formats = cfg.structuredData; const none = !formats.jsonLd && !formats.microdata && !formats.rdfa;
  const empty = cfg.loaded && gate(none && featureDisabled("extraction.structured_data"));
  const result = useDetailRequest(`sd:${props.selection.crawlId}:${focus}:${cfg.loaded && !empty}`, signal => cfg.loaded && !empty ? detail.structuredData({ urlId: focus }, signal) : Promise.resolve(undefined));
  const tree = useMemo(() => buildTree(result.value?.nodes ?? []), [result.value]);
  const all = useMemo(() => flatten(tree), [tree]);
  // Der Baum öffnet vollständig: Formate, Elemente und verschachtelte Elemente sind sofort sichtbar (UI-045).
  useEffect(() => { setExpanded(new Set(all.filter(e => e.children.length > 0).map(e => e.id))); }, [all]);
  const selected = all.find(e => e.id === selectedId);
  const sev = (s: Severity) => t(`sd-severity-${s}`);
  const renderEntry = (e: TreeEntry): ReactNode => <TreeItem key={e.id} id={e.id} textValue={`${e.label}${e.value ? ` ${e.value}` : ""}`} className="os-tree-item">
    <TreeItemContent>
      {({ hasChildItems }) => <>
        {hasChildItems ? <RacButton slot="chevron" className="os-tree-toggle"><ChevronRight size={12} aria-hidden className="os-tree-chevron" /></RacButton> : <span className="os-tree-chevron" />}
        <span className="os-sd-node" data-kind={e.kind} onContextMenu={event => { event.preventDefault(); setSelectedId(e.id); setMenu({ id: e.id, x: event.clientX, y: event.clientY }); }}>
          <Symbol severity={e.severity} label={sev(e.severity)} /><span className="os-sd-name">{e.label}</span>{e.value !== undefined && <span className="os-sd-value">{e.value}</span>}</span>
      </>}
    </TreeItemContent>
    {e.children.map(renderEntry)}
  </TreeItem>;
  if (!cfg.loaded) return <DetailLoading />;
  if (empty) return <div className="os-fach"><FachEmpty state={empty} onError={actions.reportError} /></div>;
  if (result.error) return <DetailError error={result.error} />;
  if (result.loading || !result.value) return <DetailLoading />;
  if (tree.length === 0) return <div className="os-fach"><FachEmpty state={{ code: "NOT_APPLICABLE" }} /></div>;
  const issues: SdIssueDto[] = selected ? selected.all : tree.flatMap(e => e.all);
  const counts = summary(tree);
  const menuEntry = menu ? all.find(e => e.id === menu.id) : undefined;
  const copyable = menuEntry?.kind === "item" && menuEntry.node && menuEntry.format !== "json-ld";
  return <div className="os-fach os-sd">
    <div className="os-fach-toolbar">
      <span role="status" className="os-fach-note">{t("sd-summary", { errors: counts.errors, warnings: counts.warnings })}</span>
      <Button onPress={() => actions.export({ kind: "detail", detail: "structuredDataIssues", urlIds: [focus], params: {}, sort: [] }, { kind: "ids", ids: [focus] })}>{t("detail-export")}</Button>
      {selected && <Button onPress={() => setSelectedId(undefined)}>{t("sd-show-all")}</Button>}
    </div>
    <div className="os-fach-split">
      <section className="os-fach-pane" aria-label={t("sd-tree")}>
        <div className="os-fach-pane-body" onKeyDown={event => {
          if ((event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) && selected) {
            event.preventDefault(); const rect = (event.target as HTMLElement).getBoundingClientRect(); setMenu({ id: selected.id, x: rect.left + 8, y: rect.bottom });
          }
        }}>
          <Tree aria-label={t("sd-tree")} className="os-tree" selectionMode="single" selectedKeys={selectedId ? [selectedId] : []} expandedKeys={expanded} onExpandedChange={keys => setExpanded(new Set([...keys].map(String)))}
            onSelectionChange={keys => { const first = [...keys][0]; if (first !== undefined) setSelectedId(String(first)); }}>{tree.map(renderEntry)}</Tree>
        </div>
      </section>
      <section className="os-fach-pane" aria-label={t("sd-issues")}>
        <div className="os-fach-pane-body">
          {selected?.kind === "parse-error" && selected.node && <div className="os-sd-parse" role="group" aria-label="Parse Error">
            <p><strong>Parse Error</strong> {selected.node.sourceLine ? t("sd-position", { line: selected.node.sourceLine, column: selected.node.sourceCol ?? 1 }) : null}</p>
            {selected.issues[0]?.snippet && <pre className="os-fach-code">{selected.issues[0].snippet}</pre>}
          </div>}
          {issues.length === 0 ? <p className="os-fach-note">{t("sd-no-issues")}</p> : <table className="os-fach-table" aria-label={t("sd-issues")}>
            <thead><tr><th scope="col">Validation Type</th><th scope="col">Severity</th><th scope="col">Issue</th><th scope="col">Property</th><th scope="col">ID</th><th scope="col">{t("sd-link")}</th></tr></thead>
            <tbody>{issues.map((issue, index) => <tr key={`${issue.code}:${issue.path}:${index}`} tabIndex={0} data-severity={severityOf(issue)}
              onClick={() => { const owner = ownerOf(tree, issue); if (owner) setSelectedId(owner.id); }}
              onKeyDown={event => { if (event.key === "Enter") { const owner = ownerOf(tree, issue); if (owner) setSelectedId(owner.id); } }}>
              <td>{validationType(issue)}</td><td><Symbol severity={severityOf(issue)} label={sev(severityOf(issue))} /> {sev(severityOf(issue))}</td><td>{issue.message}</td>
              <td className="os-fach-code">{issue.property ?? issue.path}</td><td className="os-fach-code">{issue.code}</td>
              <td>{issue.docUrl ? <button type="button" className="os-detail-link" onClick={event => { event.stopPropagation(); actions.openBrowser(issue.docUrl!); }}>{t("sd-doc")}</button> : null}</td></tr>)}</tbody>
          </table>}
        </div>
      </section>
    </div>
    <span ref={anchorRef} style={{ position: "fixed", left: menu?.x ?? 0, top: menu?.y ?? 0, width: 1, height: 1 }} />
    <Popover triggerRef={anchorRef} isOpen={!!menu} onOpenChange={open => { if (!open) setMenu(undefined); }} className="os-popover" placement="bottom start">
      <MenuBody label={t("detail-context")} entries={[{ id: "copy-jsonld", label: t("sd-copy-jsonld"), isDisabled: !copyable, disabledReason: !copyable ? t("sd-copy-jsonld-only") : undefined,
        onAction: () => { if (menuEntry?.node) copyText(JSON.stringify(toJsonLd(menuEntry.node), null, 2), actions.reportError); setMenu(undefined); } }]} />
    </Popover>
  </div>;
}
