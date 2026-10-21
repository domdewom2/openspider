import { useEffect, useRef, useState } from "react";
import { detail } from "../../../ipc/detail";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { useT } from "../../../i18n";
import { Banner } from "../../../ui/primitives/Feedback";
import { Select } from "../../../ui/primitives/fields";
import type { MenuEntry } from "../../../ui/primitives/Menu";
import { DetailError, DetailLoading } from "../frame/Feedback";
import { useDetailRequest } from "../frame/request";
import { TablePanel } from "../frame/TablePanel";
import type { DetailPanelProps } from "../frame/types";
import { focusOf } from "../rendered/common";
import { spellingActions } from "./actions";
import { markPieces } from "./context";

export const SPELLING_COLUMNS = ["Error", "Type", "Detail", "Suggestion", "Page Section", "Rule ID"] as const;
/** Erweiterungsspalten sind standardmäßig ausgeblendet und einblendbar (ONPAGE-157). */
export const SPELLING_HIDDEN = ["Page Section", "Rule ID"] as const;
const SECTIONS = ["Page Title", "Meta Description", "Body"] as const;
const TYPES = ["Spelling", "Grammar"] as const;

function Excerpt({ urlId, findingId, crawlId }: { urlId: number; findingId: number; crawlId?: string }) {
  const t = useT(); const first = useRef<HTMLElement>(null);
  const context = useDetailRequest(`spell-context:${crawlId}:${urlId}:${findingId}`, signal => detail.spellingContext({ urlId, findingId }, signal));
  useEffect(() => { first.current?.scrollIntoView?.({ block: "center" }); }, [context.value]);
  if (context.error) return <DetailError error={context.error} />;
  if (!context.value) return <DetailLoading />;
  const { text, start, end, suggestions } = context.value;
  return <div className="os-spell-excerpt">
    <p className="os-spell-text" lang="und">{markPieces(text, start, end).map((piece, index) => piece.mark
      ? <mark key={index} className="os-fach-mark" ref={piece.first ? first : undefined} data-first={piece.first ? "true" : undefined}>{piece.text}</mark> : <span key={index}>{piece.text}</span>)}</p>
    {suggestions.length > 0 && <p className="os-fach-note">{t("spell-suggestions")}: {suggestions.join(", ")}</p>}
  </div>;
}

export function Spelling(props: DetailPanelProps) {
  const t = useT(); const { actions } = props; const focus = focusOf(props.selection);
  const [section, setSection] = useState("all"); const [type, setType] = useState("all"); const [sort, setSort] = useState<SortKey[]>([]);
  const [finding, setFinding] = useState<{ key: number; error: string }>(); const [notice, setNotice] = useState<string>();
  useEffect(() => { setFinding(undefined); }, [focus, props.selection.crawlId, section, type]);
  const params: Record<string, string> = {}; if (section !== "all") params.section = section; if (type !== "all") params.type = type;
  const run = (message: string, action: () => Promise<void>) => void action().then(() => setNotice(message)).catch(actions.reportError);
  const contextEntries = ({ get }: { key?: number; get: (column: string) => string }): MenuEntry[] => {
    const error = get("Error"); const rule = get("Rule ID"); const kind = get("Type");
    return [
      { id: "ignore-all", label: t("spell-ignore-all"), isDisabled: !error, onAction: () => run(t("spell-done-ignore", { word: error }), () => spellingActions().ignoreWord(error, undefined)) },
      { id: "ignore-rule", label: t("spell-ignore-rule"), isDisabled: kind !== "Grammar" || !rule, disabledReason: kind !== "Grammar" ? t("spell-rule-grammar-only") : !rule ? t("spell-rule-missing") : undefined,
        onAction: () => run(t("spell-done-rule", { rule }), () => spellingActions().ignoreRule(rule)) },
      { id: "add-dictionary", label: t("spell-add-dictionary"), isDisabled: !error || kind === "Grammar", disabledReason: kind === "Grammar" ? t("spell-dictionary-spelling-only") : undefined,
        onAction: () => run(t("spell-done-dictionary", { word: error }), () => spellingActions().addToDictionary(error, undefined)) },
    ];
  };
  return <div className="os-fach os-spell">
    <div className="os-fach-toolbar">
      <Select label={t("spell-filter-type")} value={type} onChange={setType} options={[{ value: "all", label: t("spell-all") }, ...TYPES.map(value => ({ value, label: value }))]} />
      <Select label={t("spell-filter-section")} value={section} onChange={setSection} options={[{ value: "all", label: t("spell-all") }, ...SECTIONS.map(value => ({ value, label: value }))]} />
    </div>
    {notice && <Banner kind="info" onDismiss={() => setNotice(undefined)} dismissLabel={t("detail-close")}>{notice}</Banner>}
    <div className="os-fach-fill os-spell-table">
      <TablePanel {...props} title={t("detail-tab-spelling")} columns={SPELLING_COLUMNS} hiddenColumns={SPELLING_HIDDEN} plainContext contextEntries={contextEntries}
        spec={{ kind: "detail", detail: "spelling", urlIds: [focus], params, sort }} onSort={setSort}
        onSelectRow={(get, key) => setFinding(key === undefined ? undefined : { key, error: get("Error") })} />
    </div>
    <section className="os-fach-pane os-spell-context" aria-label={t("spell-context")}><h3>{t("spell-context")}</h3>
      <div className="os-fach-pane-body">{finding ? <Excerpt key={finding.key} urlId={focus} findingId={finding.key} crawlId={props.selection.crawlId} /> : <p className="os-fach-note">{t("spell-pick")}</p>}</div>
    </section>
  </div>;
}
