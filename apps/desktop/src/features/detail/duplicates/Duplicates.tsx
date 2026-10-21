import { useMemo, useRef, useState } from "react";
import { Label, Radio, RadioGroup } from "react-aria-components";
import { detail } from "../../../ipc/detail";
import type { DupPartner } from "../../../ipc/gen/DupPartner";
import type { EmptyState as EmptyStateDto } from "../../../ipc/gen/EmptyState";
import { useLanguageStore, useT } from "../../../i18n";
import { useSession } from "../../../state/session";
import { Button } from "../../../ui/primitives/Button";
import { Banner } from "../../../ui/primitives/Feedback";
import { DetailError, DetailLoading } from "../frame/Feedback";
import { useDetailRequest } from "../frame/request";
import type { DetailPanelProps } from "../frame/types";
import { FachEmpty, featureDisabled, focusOf, gate, useFachConfig } from "../rendered/common";
import { diffWords, tokenize, type Token } from "./diff";
import "./duplicates.css";

export type DupFilter = "exact" | "near" | "semantic";
export const DUP_FILTERS: readonly DupFilter[] = ["exact", "near", "semantic"];
export const NEAR_STEP = "near_duplicates";

/** Similarity absteigend, bei Gleichstand Address aufsteigend (ONPAGE-145). */
export function sortPartners(partners: readonly DupPartner[]): DupPartner[] {
  return [...partners].sort((a, b) => (b.similarity ?? 100) - (a.similarity ?? 100) || a.address.localeCompare(b.address, "en"));
}
interface Line { changed: boolean; parts: { text: string; mark: boolean }[] }
/** Zerlegt einen Text in Zeilen mit markierten Token; Leerraum zwischen Token bleibt erhalten. */
export function textLines(text: string, tokens: readonly Token[], marks: readonly boolean[]): Line[] {
  const lines: Line[] = []; let offset = 0; let cursor = 0;
  for (const raw of text.split("\n")) {
    const end = offset + raw.length; const parts: Line["parts"] = []; let at = offset; let changed = false;
    while (cursor < tokens.length && tokens[cursor]!.start < end) {
      const token = tokens[cursor]!; const mark = !!marks[cursor];
      if (token.start > at) parts.push({ text: text.slice(at, token.start), mark: false });
      parts.push({ text: token.text, mark }); changed ||= mark; at = token.end; cursor++;
    }
    if (at < end) parts.push({ text: text.slice(at, end), mark: false });
    lines.push({ changed, parts }); offset = end + 1;
  }
  return lines;
}

function TextColumn({ title, text, tokens, marks, kind, scroller, onScroll }: { title: string; text: string; tokens: Token[]; marks: boolean[]; kind: "removed" | "added" | "plain";
  scroller: React.RefObject<HTMLDivElement | null>; onScroll: () => void }) {
  const lines = useMemo(() => textLines(text, tokens, marks), [text, tokens, marks]);
  return <section className="os-fach-pane os-dup-text" aria-label={title}><h3>{title}</h3>
    <div className="os-fach-pane-body" ref={scroller} onScroll={onScroll} tabIndex={0} role="region" aria-label={title}>
      {lines.map((line, index) => <p key={index} className="os-dup-line" data-diff={line.changed ? kind : undefined}>
        <span className="os-dup-margin" aria-hidden>{line.changed ? (kind === "added" ? "+" : "−") : ""}</span>
        <span>{line.parts.map((part, i) => part.mark ? (kind === "added" ? <ins key={i}>{part.text}</ins> : <del key={i}>{part.text}</del>) : <span key={i}>{part.text}</span>)}</span>
      </p>)}
    </div></section>;
}

export function Duplicates(props: DetailPanelProps) {
  const t = useT(); const lang = useLanguageStore(s => s.lang); const cfg = useFachConfig(props.selection.crawlId);
  const { actions } = props; const focus = focusOf(props.selection);
  const analysis = useSession(s => s.analysis);
  const [filter, setFilter] = useState<DupFilter>("exact"); const [selected, setSelected] = useState<{ key: string; partner: DupPartner }>();
  const left = useRef<HTMLDivElement>(null); const right = useRef<HTMLDivElement>(null); const syncing = useRef(false);
  const step = analysis?.steps.find(s => s.step === NEAR_STEP);
  const nearStale = !step || step.state !== "done" || step.stale;
  const empty: EmptyStateDto | null = !cfg.loaded ? null : filter === "near"
    ? gate(!cfg.nearDuplicates && featureDisabled("duplicates.near_enabled"), cfg.nearDuplicates && nearStale && { code: "NEEDS_CRAWL_ANALYSIS", arg: NEAR_STEP, action: { kind: "startAnalysis" } })
    : filter === "exact" ? gate(!cfg.exactDuplicates && featureDisabled("duplicates.exact_enabled")) : gate(featureDisabled("embeddings"));
  const crawl = props.selection.crawlId; const listKey = `dup:${crawl}:${focus}:${filter}`;
  const list = useDetailRequest(listKey, signal => empty || filter === "semantic" ? Promise.resolve(undefined) : detail.duplicates({ urlId: focus, kind: filter }, signal));
  const key = `${listKey}:${selected?.partner.urlId}`;
  const partner = selected?.key.startsWith(listKey) ? selected.partner : undefined;
  const other = useDetailRequest(`dup-text:${key}`, signal => partner && !empty ? detail.duplicates({ urlId: partner.urlId, kind: filter === "near" ? "near" : "exact" }, signal) : Promise.resolve(undefined));
  const partners = useMemo(() => sortPartners(list.value?.partners ?? []), [list.value]);
  const leftText = list.value?.text ?? ""; const rightText = other.value?.text ?? "";
  const compare = useMemo(() => {
    if (!partner || !list.value || !other.value) return undefined;
    const a = tokenize(leftText); const b = tokenize(rightText);
    return { a, b, diff: diffWords(a.map(x => x.text), b.map(x => x.text)) };
  }, [partner, list.value, other.value, leftText, rightText]);
  const sync = (from: React.RefObject<HTMLDivElement | null>, to: React.RefObject<HTMLDivElement | null>) => () => {
    const source = from.current; const target = to.current; if (!source || !target || syncing.current) return;
    syncing.current = true;
    const ratio = source.scrollTop / Math.max(1, source.scrollHeight - source.clientHeight);
    target.scrollTop = ratio * Math.max(0, target.scrollHeight - target.clientHeight);
    requestAnimationFrame(() => { syncing.current = false; });
  };
  if (!cfg.loaded) return <DetailLoading />;
  const matchType = (p: DupPartner) => filter === "near" ? t("dup-match-near") : (p.similarity ?? 100) >= 100 ? t("dup-match-exact") : t("dup-match-near");
  const rowKeys = (index: number) => (event: React.KeyboardEvent) => {
    const rows = [...(event.currentTarget.parentElement?.querySelectorAll<HTMLElement>("[role=row][tabindex]") ?? [])];
    if (event.key === "ArrowDown") { event.preventDefault(); rows[index + 1]?.focus(); }
    if (event.key === "ArrowUp") { event.preventDefault(); rows[index - 1]?.focus(); }
    if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setSelected({ key: `${listKey}:${partners[index]!.urlId}`, partner: partners[index]! }); }
  };
  return <div className="os-fach os-dup">
    <div className="os-fach-toolbar">
      <RadioGroup className="os-radio-group" orientation="horizontal" value={filter} onChange={value => { setFilter(value as DupFilter); setSelected(undefined); }}>
        <Label className="os-label">{t("dup-filter")}</Label>
        <div className="os-radio-items">{DUP_FILTERS.map(value => <Radio key={value} value={value} className="os-radio" isDisabled={value === "semantic"}>
          <span className="os-radio-dot" aria-hidden /><span title={value === "semantic" ? t("dup-semantic-unavailable") : undefined}>{t(`dup-filter-${value}`)}</span></Radio>)}</div>
      </RadioGroup>
      <Button isDisabled={!!empty || filter === "semantic"} onPress={() => actions.export({ kind: "detail", detail: "duplicates", urlIds: [focus], params: { kind: filter }, sort: [] }, { kind: "ids", ids: [focus] })}>{t("detail-export")}</Button>
    </div>
    {empty ? <FachEmpty state={empty} onError={actions.reportError} /> : list.error ? <DetailError error={list.error} /> : list.loading || !list.value ? <DetailLoading /> :
      partners.length === 0 ? <FachEmpty state={{ code: "NO_MATCH" }} /> : <>
        <div className="os-fach-pane os-dup-list">
          <table className="os-fach-table" role="grid" aria-label={t("dup-list")} aria-rowcount={partners.length + 1}>
            <thead><tr role="row"><th scope="col" role="columnheader">Address</th><th scope="col" role="columnheader" className="os-fach-num">Similarity</th><th scope="col" role="columnheader">Match Type</th></tr></thead>
            <tbody>{partners.map((p, index) => <tr key={p.urlId} role="row" tabIndex={index === 0 || selected?.partner.urlId === p.urlId ? 0 : -1} aria-selected={selected?.partner.urlId === p.urlId} data-selected={selected?.partner.urlId === p.urlId}
              onClick={() => setSelected({ key: `${listKey}:${p.urlId}`, partner: p })} onKeyDown={rowKeys(index)}>
              <td role="gridcell">{p.address}</td><td role="gridcell" className="os-fach-num">{new Intl.NumberFormat(lang).format(p.similarity ?? 100)}</td><td role="gridcell">{matchType(p)}</td></tr>)}</tbody>
          </table>
        </div>
        {partner ? (other.error ? <DetailError error={other.error} /> : other.loading || !compare ? <DetailLoading /> : <>
          <p className="os-fach-note" role="status">{t("dup-summary", { removed: compare.diff.removedCount, added: compare.diff.addedCount })}</p>
          <div className="os-fach-split os-dup-compare">
            <TextColumn title={t("dup-text-this")} text={leftText} tokens={compare.a} marks={compare.diff.removed} kind="removed" scroller={left} onScroll={sync(left, right)} />
            <TextColumn title={partner.address} text={rightText} tokens={compare.b} marks={compare.diff.added} kind="added" scroller={right} onScroll={sync(right, left)} />
          </div></>) : <Banner kind="info">{t("dup-pick")}</Banner>}
      </>}
  </div>;
}
