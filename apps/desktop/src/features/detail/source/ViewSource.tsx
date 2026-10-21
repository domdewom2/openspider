import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { detail } from "../../../ipc/detail";
import { isSilentError } from "../../../ipc/errors";
import type { DiffHunks } from "../../../ipc/gen/DiffHunks";
import { useT } from "../../../i18n";
import { useAction } from "../../../ui/actions";
import { Button } from "../../../ui/primitives/Button";
import { Banner, ProgressBar } from "../../../ui/primitives/Feedback";
import { Checkbox, RadioGroup } from "../../../ui/primitives/fields";
import { DetailError, DetailLoading } from "../frame/Feedback";
import type { DetailPanelProps } from "../frame/types";
import { FachEmpty, fileStem, focusOf, saveFile, useFachConfig, useOpStatus } from "../rendered/common";
import { CodePane, type CodePaneHandle } from "./CodePane";
import { useDocLoader } from "./loader";
import { diffMarks, findSnippet, wordRanges, type SearchResult } from "./model";
import { useSourceTarget } from "./navigation";
import "./source.css";

type Side = "left" | "right";
interface Anchors { left: number; right: number }

export function ViewSource(props: DetailPanelProps) {
  const t = useT(); const cfg = useFachConfig(props.selection.crawlId); const { actions } = props; const focus = focusOf(props.selection); const crawlId = props.selection.crawlId;
  const rendering = props.rendering || cfg.rendering;
  const originalOk = props.htmlStored || cfg.storeHtml; const renderedOk = rendering && cfg.storeRenderedHtml;
  const [mode, setMode] = useState<"html" | "text">("html"); const [diff, setDiff] = useState(false); const [pretty, setPretty] = useState(false); const [sync, setSync] = useState(true);
  const [query, setQuery] = useState(""); const [regex, setRegex] = useState(false); const [active, setActive] = useState<Side>("left");
  const [search, setSearch] = useState<SearchResult & { index: number }>({ matches: [], total: 0, index: -1 });
  const [anchors, setAnchors] = useState<Anchors[]>([]); const [changeIndex, setChangeIndex] = useState(-1); const [notice, setNotice] = useState<string>();
  const left = useRef<CodePaneHandle>(null); const right = useRef<CodePaneHandle>(null); const root = useRef<HTMLDivElement>(null); const searchBox = useRef<HTMLInputElement>(null);
  const syncing = useRef(false);
  const showLeft = cfg.loaded && (originalOk || rendering || renderedOk); const showRight = cfg.loaded && rendering;
  const leftDoc = useDocLoader(left, { enabled: cfg.loaded && originalOk, urlId: focus, crawlId, which: "original", mode, pretty });
  const rightDoc = useDocLoader(right, { enabled: cfg.loaded && renderedOk, urlId: focus, crawlId, which: "rendered", mode, pretty });
  const bothReady = leftDoc.status === "ready" && rightDoc.status === "ready";

  // --- Diff: Berechnung im Kern als Operation, hier nur Markierung (UI-041) ---
  const diffKey = `${crawlId}:${focus}:${mode}`;
  const [diffOp, setDiffOp] = useState<{ key: string; opId: string }>();
  useEffect(() => {
    if (!(diff && bothReady)) { setDiffOp(undefined); return; }
    const controller = new AbortController();
    void detail.sourceDiff({ urlId: focus, mode: mode === "html" ? "lines" : "words" }, controller.signal)
      .then(handle => { if (!controller.signal.aborted) setDiffOp({ key: diffKey, opId: handle.opId }); })
      .catch(error => { if (!controller.signal.aborted && !isSilentError(error)) actions.reportError(error); });
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diff, bothReady, diffKey]);
  const op = useOpStatus(diffOp?.key === diffKey ? diffOp.opId : undefined);
  const hunks = op?.state === "done" && op.result ? op.result as unknown as DiffHunks : undefined;
  useEffect(() => {
    const l = left.current; const r = right.current; if (!l || !r) return;
    if (!diff || !hunks || !bothReady) { l.setOverlay({ lineKinds: new Map(), wordMarks: [] }); r.setOverlay({ lineKinds: new Map(), wordMarks: [] }); setAnchors([]); setChangeIndex(-1); return; }
    const marks = diffMarks(hunks);
    if (mode === "html") {
      l.setOverlay({ lineKinds: marks.leftLines, wordMarks: [] }); r.setOverlay({ lineKinds: marks.rightLines, wordMarks: [] });
      setAnchors(marks.changes.map(c => ({ left: l.lineStart(c.left), right: r.lineStart(c.right) })));
    } else {
      const a = wordRanges(l.getText(), marks.leftWords, "removed", pos => l.lineAt(pos)); const b = wordRanges(r.getText(), marks.rightWords, "added", pos => r.lineAt(pos));
      l.setOverlay({ lineKinds: a.lines, wordMarks: a.ranges }); r.setOverlay({ lineKinds: b.lines, wordMarks: b.ranges });
      const at = (tokens: typeof a.tokens, index: number, text: string) => tokens[index - 1]?.start ?? text.length;
      setAnchors(marks.changes.map(c => ({ left: at(a.tokens, c.left, l.getText()), right: at(b.tokens, c.right, r.getText()) })));
    }
    setChangeIndex(-1);
  }, [hunks, bothReady, diff, mode]);
  const go = useCallback((delta: number) => {
    if (anchors.length === 0) return;
    const next = ((changeIndex + delta) % anchors.length + anchors.length) % anchors.length;
    setChangeIndex(next); left.current?.scrollTo(anchors[next]!.left); right.current?.scrollTo(anchors[next]!.right);
  }, [anchors, changeIndex]);
  useAction("source.nextDiff", () => { if (anchors.length === 0) return false; go(1); });
  useAction("source.prevDiff", () => { if (anchors.length === 0) return false; go(-1); });

  // --- Suche in der aktiven Seite (UI-041) ---
  const pane = (side: Side) => side === "left" ? left : right;
  const jump = useCallback((side: Side, result: SearchResult, index: number) => {
    const match = result.matches[index]; if (!match) return;
    pane(side).current?.setOverlay({ current: index }); pane(side).current?.scrollTo(match.from, match);
    setSearch({ ...result, index });
  }, []);
  useEffect(() => {
    const timer = setTimeout(() => {
      for (const side of ["left", "right"] as const) pane(side).current?.setOverlay({ matches: [], current: -1 });
      const current = pane(active).current; if (!current) return;
      const result = current.findMatches(query, regex);
      current.setOverlay({ matches: result.matches, current: -1 });
      if (result.matches.length > 0) jump(active, result, 0); else setSearch({ ...result, index: -1 });
    }, 150);
    return () => clearTimeout(timer);
  }, [query, regex, active, leftDoc.status, rightDoc.status, mode, jump]);
  const step = (delta: number) => { if (search.matches.length === 0) return; jump(active, search, ((search.index + delta) % search.matches.length + search.matches.length) % search.matches.length); };
  useAction("search.focus", () => { if (!root.current?.contains(document.activeElement)) return false; searchBox.current?.focus(); searchBox.current?.select(); });

  // --- „Im gerenderten HTML anzeigen“ (UI-048): markiert die Stelle in der gerenderten Fassung ---
  const target = useSourceTarget(s => s.target); const handled = useRef(0);
  useEffect(() => {
    if (!target || target.id === handled.current || target.urlId !== focus || (target.crawlId ?? crawlId) !== crawlId) return;
    if (!renderedOk) { handled.current = target.id; setNotice(t("source-target-unavailable")); return; }
    setMode("html"); setActive("right");
    if (rightDoc.status !== "ready" || mode !== "html") return;
    handled.current = target.id;
    const r = right.current; if (!r) return;
    const range = findSnippet(r.getText(), target.snippet);
    if (range) { r.setOverlay({ target: range }); r.scrollTo(range.from, range); setNotice(t("source-target-found", { selector: target.selector })); }
    else setNotice(t("source-target-missing", { selector: target.selector }));
  }, [target, focus, crawlId, renderedOk, rightDoc.status, mode, t]);

  const exportActive = async () => {
    const text = pane(active).current?.getText() ?? ""; const address = String((await detail.urlDetails({ urlId: focus })).find(row => row.name === "Address")?.value ?? focus);
    saveFile(`${fileStem(address)}${active === "right" ? ".rendered" : ""}.${mode === "html" ? "html" : "txt"}`, text, mode === "html" ? "text/html" : "text/plain");
  };
  const onScroll = (from: Side) => () => {
    if (!sync || syncing.current) return;
    const a = pane(from).current?.scroller(); const b = pane(from === "left" ? "right" : "left").current?.scroller(); if (!a || !b) return;
    syncing.current = true; b.scrollTop = a.scrollTop / Math.max(1, a.scrollHeight - a.clientHeight) * Math.max(0, b.scrollHeight - b.clientHeight); b.scrollLeft = a.scrollLeft;
    requestAnimationFrame(() => { syncing.current = false; });
  };

  if (!cfg.loaded) return <DetailLoading />;
  if (!originalOk && !renderedOk) return <div className="os-fach"><FachEmpty state={{ code: "HTML_NOT_STORED", action: { kind: "openSetting", anchor: "extraction.store_html" } }} onError={actions.reportError} /></div>;
  const progress = (doc: typeof leftDoc) => doc.status === "loading" && doc.lines > 0 ? <ProgressBar label={t("source-loading")} value={doc.loaded} max={doc.lines} /> : null;
  const diffReason = !showRight || !renderedOk ? t("source-diff-needs-rendered") : !originalOk ? t("source-diff-needs-original") : undefined;
  const unit = mode === "html" ? t("source-unit-lines") : t("source-unit-words");
  const pane1 = (side: Side, title: string, doc: typeof leftDoc, available: boolean, emptyCode: "HTML_NOT_STORED" | "RENDERED_HTML_NOT_STORED") => <section className="os-fach-pane os-src-side" aria-label={title} data-side={side}>
    <h3>{title}{doc.status === "ready" && <span className="os-fach-note"> · {t("source-size", { lines: doc.lines, kb: Math.max(1, Math.round(doc.bytes / 1024)) })}</span>}</h3>
    {progress(doc)}
    {doc.status === "error" ? <DetailError error={doc.error} /> : null}
    {available ? <CodePane ref={pane(side)} label={title} wrap={mode === "text"} highlight={mode === "html"} onFocus={() => setActive(side)} onScroll={onScroll(side)} />
      : <FachEmpty state={{ code: emptyCode, action: { kind: "openSetting", anchor: emptyCode === "HTML_NOT_STORED" ? "extraction.store_html" : "extraction.store_rendered_html" } }} onError={actions.reportError} />}
  </section>;
  return <div className="os-fach os-src" ref={root}>
    <div className="os-fach-toolbar">
      <RadioGroup label={t("source-filter")} orientation="horizontal" value={mode} onChange={value => setMode(value as "html" | "text")} options={[{ value: "html", label: "HTML" }, { value: "text", label: t("source-text") }]} />
      <span title={diffReason}><Checkbox isSelected={diff} onChange={setDiff} isDisabled={!!diffReason}>{t("source-diff")}</Checkbox></span>
      <Checkbox isSelected={pretty} onChange={setPretty} isDisabled={mode === "text"}>{t("source-pretty")}</Checkbox>
      <Checkbox isSelected={sync} onChange={setSync} isDisabled={!showRight}>{t("source-sync")}</Checkbox>
      <Button onPress={() => go(-1)} isDisabled={anchors.length === 0} aria-label={t("source-prev-diff")}><ChevronUp size={14} aria-hidden /></Button>
      <Button onPress={() => go(1)} isDisabled={anchors.length === 0} aria-label={t("source-next-diff")}><ChevronDown size={14} aria-hidden /></Button>
      {diff && op?.state === "done" && hunks && <span role="status" className="os-src-summary" data-testid="diff-summary">{t("source-summary", { added: hunks.added, removed: hunks.removed, unit })}</span>}
      {anchors.length > 0 && <span role="status" className="os-fach-note" data-testid="change-position">{changeIndex < 0 ? t("source-changes", { total: anchors.length }) : t("source-change-position", { index: changeIndex + 1, total: anchors.length })}</span>}
      <Button onPress={() => void exportActive()}>{t("source-export", { ext: mode === "html" ? ".html" : ".txt" })}</Button>
    </div>
    <div className="os-fach-toolbar os-src-search">
      <label className="os-field"><span className="os-label">{t("source-search")}</span>
        <input ref={searchBox} className="os-input os-fach-search" type="search" value={query} onChange={event => setQuery(event.target.value)} aria-invalid={search.error ? true : undefined}
          onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); step(event.shiftKey ? -1 : 1); } }} /></label>
      <Checkbox isSelected={regex} onChange={setRegex}>{t("source-regex")}</Checkbox>
      <Button onPress={() => step(-1)} isDisabled={search.matches.length === 0}>{t("source-prev-match")}</Button>
      <Button onPress={() => step(1)} isDisabled={search.matches.length === 0}>{t("source-next-match")}</Button>
      <span role="status" className="os-fach-note" data-testid="match-count">{query === "" ? "" : search.error ? t("source-regex-invalid") : t("source-matches", { index: search.index + 1, total: search.total })}</span>
    </div>
    {diff && op && op.state === "running" && <ProgressBar label={t("source-diffing")} value={op.total ? op.done : null} max={op.total ?? 100} />}
    {op?.state === "failed" && <Banner kind="error">{t("source-diff-failed")}</Banner>}
    {notice && <Banner onDismiss={() => setNotice(undefined)} dismissLabel={t("detail-close")}>{notice}</Banner>}
    <div className={`os-fach-split os-src-panes${showLeft && showRight ? "" : " os-src-single"}`}>
      {showLeft && pane1("left", t("source-original"), leftDoc, originalOk, "HTML_NOT_STORED")}
      {showRight && pane1("right", t("source-rendered"), rightDoc, renderedOk, "RENDERED_HTML_NOT_STORED")}
    </div>
  </div>;
}
