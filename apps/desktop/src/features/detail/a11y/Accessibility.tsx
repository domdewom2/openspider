import { useState } from "react";
import { detail } from "../../../ipc/detail";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { useT } from "../../../i18n";
import { RadioGroup } from "../../../ui/primitives/fields";
import type { MenuEntry } from "../../../ui/primitives/Menu";
import { useDetailRequest } from "../frame/request";
import { TablePanel } from "../frame/TablePanel";
import type { DetailPanelProps } from "../frame/types";
import { copyText, FachEmpty, featureDisabled, focusOf, gate, needsRendering, useFachConfig } from "../rendered/common";
import { DetailLoading } from "../frame/Feedback";
import { showInSource } from "../source/navigation";
import { A11Y_RESULTS, DEFAULT_RULE_SORT, NODE_COLUMNS, RULE_COLUMNS, type A11yResult } from "./model";

interface Rule { id: string; result: string; nodes: string }
interface NodeRow { key?: number; address: string; selector: string; html: string }

function NodeInfo({ urlId, node, crawlId }: { urlId: number; node: NodeRow; crawlId?: string }) {
  const t = useT();
  // Die Kurzbeschreibung kommt aus `detail.a11yNode`; fehlt der Befehl noch, bleiben die Tabellenwerte.
  const extra = useDetailRequest(`a11y-node:${crawlId}:${urlId}:${node.key}`, async signal => { try { return node.key === undefined ? undefined : await detail.a11yNode({ urlId, nodeId: node.key }, signal); } catch { return undefined; } });
  return <section className="os-fach-pane os-a11y-node" aria-label={t("a11y-node")}><h3>{t("a11y-node")}</h3>
    <div className="os-fach-pane-body"><dl className="os-a11y-dl">
      <dt>Location on Page</dt><dd className="os-fach-code">{node.selector}</dd>
      <dt>HTML</dt><dd className="os-fach-code">{node.html}</dd>
      {extra.value?.impact && <><dt>Impact</dt><dd>{extra.value.impact}</dd></>}
      {extra.value?.summaryKey && <><dt>{t("a11y-summary")}</dt><dd>{t(extra.value.summaryKey)}</dd></>}
    </dl></div></section>;
}

export function Accessibility(props: DetailPanelProps) {
  const t = useT(); const cfg = useFachConfig(props.selection.crawlId); const { actions } = props; const focus = focusOf(props.selection);
  const [result, setResult] = useState<A11yResult>("Violation");
  const [rulesSort, setRulesSort] = useState<SortKey[]>(DEFAULT_RULE_SORT); const [nodesSort, setNodesSort] = useState<SortKey[]>([]);
  const [rule, setRule] = useState<Rule>(); const [node, setNode] = useState<NodeRow>();
  const rendering = props.rendering || cfg.rendering;
  const empty = cfg.loaded && gate(!rendering && needsRendering(), rendering && !cfg.accessibility && featureDisabled("accessibility", "extraction.accessibility"));
  if (!cfg.loaded) return <DetailLoading />;
  if (empty) return <div className="os-fach"><FachEmpty state={empty} onError={actions.reportError} /></div>;
  const emptyRule = rule && (rule.result === "Passed" || rule.result === "Inapplicable");
  const nodeEntries = ({ get }: { key?: number; get: (column: string) => string }): MenuEntry[] => {
    const selector = get("Location on Page"); const html = get("HTML"); const address = get("Address");
    return [
      { id: "show-rendered", label: t("a11y-show-rendered"), isDisabled: !cfg.storeRenderedHtml, disabledReason: !cfg.storeRenderedHtml ? t("a11y-rendered-not-stored") : undefined, onAction: () => showInSource({ urlId: focus, crawlId: props.selection.crawlId, which: "rendered", snippet: html, selector }) },
      { id: "show-browser", label: t("a11y-show-browser"), isDisabled: !/^https?:\/\//i.test(address), disabledReason: !/^https?:\/\//i.test(address) ? t("a11y-no-address") : undefined,
        onAction: () => { copyText(selector, actions.reportError); actions.openBrowser(address); } },
      { id: "copy-selector", label: t("a11y-copy-selector"), isDisabled: !selector, onAction: () => copyText(selector, actions.reportError) },
    ];
  };
  return <div className="os-fach os-a11y">
    <div className="os-fach-toolbar">
      <RadioGroup label={t("a11y-result")} orientation="horizontal" value={result} onChange={value => { setResult(value as A11yResult); setRule(undefined); setNode(undefined); }}
        options={A11Y_RESULTS.map(value => ({ value, label: t(`a11y-result-${value}`) }))} />
    </div>
    <div className="os-fach-split">
      <section className="os-fach-pane" aria-label={t("a11y-rules")}><h3>{t("a11y-rules")}</h3>
        <div className="os-fach-pane-body os-fach-fill">
          <TablePanel {...props} title={t("a11y-rules")} columns={RULE_COLUMNS} plainContext
            spec={{ kind: "detail", detail: "a11yRules", urlIds: [focus], params: result === "All" ? {} : { result }, sort: rulesSort }} onSort={setRulesSort}
            onSelectRow={get => { setRule({ id: get("Issue"), result: get("Result"), nodes: get("Nodes") }); setNode(undefined); }} />
        </div>
      </section>
      <section className="os-fach-pane" aria-label={t("a11y-nodes")}><h3>{t("a11y-nodes")}</h3>
        <div className="os-fach-pane-body os-fach-fill">
          {!rule ? <p className="os-fach-note">{t("a11y-pick-rule")}</p> : emptyRule ? <p className="os-fach-note" role="status" data-testid="a11y-no-elements">{t("a11y-no-elements")}</p> :
            <TablePanel {...props} title={t("a11y-nodes")} columns={NODE_COLUMNS} plainContext contextEntries={nodeEntries}
              spec={{ kind: "detail", detail: "a11yNodes", urlIds: [focus], params: { rule: rule.id, ...(result === "All" ? {} : { result }) }, sort: nodesSort }} onSort={setNodesSort}
              onSelectRow={(get, key) => setNode({ key, address: get("Address"), selector: get("Location on Page"), html: get("HTML") })} />}
        </div>
      </section>
    </div>
    {node && <NodeInfo urlId={focus} node={node} crawlId={props.selection.crawlId} />}
  </div>;
}
