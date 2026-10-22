import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { compare } from "../../ipc/compare";
import { crawls } from "../../ipc/crawls";
import type { CrawlListing } from "../../ipc/gen/CrawlListing";
import type { MappingRuleDto } from "../../ipc/gen/MappingRuleDto";
import type { RuleTrace } from "../../ipc/gen/RuleTrace";
import { describeError, toCmdError } from "../../ipc/errors";
import { Dialog, DialogFooter } from "../../ui/primitives/Dialog";
import { Button } from "../../ui/primitives/Button";
import { useMainMode } from "../help/state";
import { closeComparison, openComparison, showComparison, swapComparison, useComparison } from "./model";
import "./compare.css";

export function ComparisonControls() {
  const t = useT(); const state = useComparison();
  const [rows, setRows] = useState<CrawlListing[]>([]);
  const [current, setCurrent] = useState(""); const [previous, setPrevious] = useState("");
  const [rules, setRules] = useState<MappingRuleDto[]>([]); const [input, setInput] = useState("");
  const [trace, setTrace] = useState<RuleTrace>(); const [checking, setChecking] = useState(false); const [error, setError] = useState<unknown>();
  const mode = useMainMode(s => s.mode);
  useEffect(() => { if (mode === "compare") showComparison(); }, [mode]);
  useEffect(() => {
    const onAction=(event:Event)=>{if((event as CustomEvent<{id:string}>).detail?.id==="mode.compare")showComparison();};
    document.addEventListener("os:action",onAction);
    return ()=>document.removeEventListener("os:action",onAction);
  }, []);
  useEffect(() => {
    if (!state.dialog) return;
    const controller = new AbortController();
    setError(undefined);
    setRules(state.session?.mapping ?? []);
    void crawls.list({ show: "crawls" }, controller.signal).then(list => {
      if (controller.signal.aborted) return;
      const sorted = [...list].sort((a,b) => b.crawl.startedAt.localeCompare(a.crawl.startedAt) || a.crawl.crawlId.localeCompare(b.crawl.crawlId));
      setRows(sorted); setCurrent(state.session?.current.crawlId ?? sorted[0]?.crawl.crawlId ?? ""); setPrevious(state.session?.previous.crawlId ?? sorted[1]?.crawl.crawlId ?? "");
    }).catch(e => { if (!controller.signal.aborted) setError(e); });
    return () => controller.abort();
  }, [state.dialog]);
  useEffect(() => {
    if (!state.dialog) return;
    const controller = new AbortController(); setChecking(true); setTrace(undefined); setError(undefined);
    void compare.testMapping({ rules, input }, controller.signal).then(result => { if (!controller.signal.aborted) setTrace(result); })
      .catch(e => { if (!controller.signal.aborted) setError(e); }).finally(() => { if (!controller.signal.aborted) setChecking(false); });
    return () => controller.abort();
  }, [state.dialog, JSON.stringify(rules), input]);
  const invalid = checking || !trace || trace.steps.some(step => !!step.error) || !!error;
  const patch = (index: number, value: Partial<MappingRuleDto>) => setRules(r => r.map((rule,i) => i === index ? { ...rule, ...value } : rule));
  const move = (index: number, delta: number) => setRules(r => { const next = [...r]; [next[index], next[index + delta]] = [next[index + delta]!, next[index]!]; return next; });
  return <div className="os-compare-controls">
    <Button onPress={showComparison} isDisabled={state.busy}>{t("compare-open")}</Button>
    {state.session && <><span title={`${state.session.current.name} / ${state.session.previous.name}`}>{state.session.current.name} / {state.session.previous.name}</span>
      <Button onPress={() => void swapComparison()} isDisabled={state.busy}>{t("compare-swap")}</Button>
      <Button onPress={() => void closeComparison()} isDisabled={state.busy}>{t("compare-close")}</Button></>}
    {state.busy && <span role="status">{t("compare-loading")}</span>}
    {state.error != null && <p role="alert">{describeError(t, toCmdError(state.error)).text}</p>}
    <Dialog title={t("compare-open")} isOpen={state.dialog} onOpenChange={dialog => { if (!state.busy) useComparison.setState({ dialog }); }}
      footer={<DialogFooter okLabel={t("compare-run")} cancelLabel={t("compare-cancel")} okDisabled={invalid || !current || !previous || current === previous || state.busy}
        onCancel={() => useComparison.setState({ dialog: false })} onOk={() => void openComparison({ current, previous, mapping: rules, elements: state.session?.elements ?? [], contentThresholdPct: state.session?.contentThresholdPct })} />}>
      <div className="os-compare-form">{(["Current", "Previous"] as const).map(side => <label key={side}>{side}<select aria-label={side} value={side === "Current" ? current : previous} onChange={e => (side === "Current" ? setCurrent : setPrevious)(e.target.value)}>
        <option value="">{t("compare-select")}</option>{rows.map(row => <option key={row.crawl.crawlId} value={row.crawl.crawlId}>{row.crawl.name} — {row.crawl.startedAt}</option>)}
      </select></label>)}</div>
      <p>{t("compare-order")}</p>
      <fieldset><legend>{t("compare-mapping")}</legend>{rules.map((rule,i) => <div key={i} className="os-compare-rule">
        <label>{t("compare-pattern")} {i + 1}<input value={rule.pattern} onChange={e => patch(i, { pattern: e.target.value })} /></label>
        <label>{t("compare-replacement")} {i + 1}<input value={rule.replacement} onChange={e => patch(i, { replacement: e.target.value })} /></label>
        <Button aria-label={`${t("compare-up")} ${i + 1}`} isDisabled={i === 0} onPress={() => move(i,-1)}>↑</Button>
        <Button aria-label={`${t("compare-down")} ${i + 1}`} isDisabled={i === rules.length-1} onPress={() => move(i,1)}>↓</Button>
        <Button aria-label={`${t("compare-remove")} ${i + 1}`} onPress={() => setRules(r => r.filter((_,n) => n !== i))}>×</Button>
      </div>)}<Button onPress={() => setRules(r => [...r, { pattern: "", replacement: "" }])}>{t("compare-add-rule")}</Button></fieldset>
      <section aria-label={t("compare-test")}><h3>{t("compare-test")}</h3><label>{t("compare-input")}<input value={input} onChange={e => setInput(e.target.value)} /></label>
        <ol>{trace?.steps.map(step => <li key={step.index}><output>{step.after}</output>{step.error && <p role="alert">{step.error}</p>}</li>)}</ol>
        <output aria-label={t("compare-result")}>{trace?.output}</output></section>
      {error != null && <p role="alert">{describeError(t, toCmdError(error)).text}</p>}
    </Dialog>
  </div>;
}
