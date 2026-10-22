import { downloadComparison } from "./export";
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { Button } from "../../ui/primitives/Button";
import { ELEMENT_FILTERS, CATEGORIES, comparisonRows, navigateComparison, selectComparisonUrl, setComparisonSide, type ComparisonPage, useComparison } from "./model";
export function ComparisonTabs() {
  const t=useT(); const state=useComparison();
  const tabs=[...new Set(state.snapshot?.overview.map(row=>row.tab) ?? [])].filter(tab=>tab!=="Change Detection");tabs.push("Change Detection");
  return <div role="tablist" aria-label={t("compare-tabs")} className="os-compare-tabs">{tabs.map(tab=><button type="button" role="tab" aria-selected={state.query.tab===tab} key={tab} onClick={()=>navigateComparison(tab,"All")}>{tab}</button>)}</div>;
}
export function ComparisonTable() {
  const t=useT(); const state=useComparison(); const [page,setPage]=useState<ComparisonPage>(); const [error,setError]=useState<unknown>();const [offset,setOffset]=useState(0);const [exporting,setExporting]=useState(false);
  const queryKey=JSON.stringify(state.query);
  useEffect(()=>{setOffset(0);},[queryKey,state.revision]);
  useEffect(()=>{
    const controller=new AbortController();setPage(undefined);setError(undefined);
    void comparisonRows(state.query,offset,controller.signal).then(result=>{if(!controller.signal.aborted)setPage(result);}).catch(e=>{if(!controller.signal.aborted)setError(e);});
    return ()=>controller.abort();
  },[queryKey,state.revision,offset]);
  const changed=state.query.tab==="Change Detection";
  const filters=changed ? ["All",...(state.session?.elements ?? []).map(e=>ELEMENT_FILTERS[e] ?? e)] : [...new Set(state.snapshot?.overview.filter(r=>r.tab===state.query.tab).map(r=>r.filter) ?? ["All"])];
  return <section className="os-compare-table" aria-label={t("compare-table")}>
    <div className="os-compare-form"><label>{t("compare-filter")}<select aria-label={t("compare-filter")} value={state.query.filter} onChange={e=>{navigateComparison(state.query.tab,e.target.value);if(changed && e.target.value!=="All")useComparison.setState(s=>({query:{...s.query,class:"changed"}}));}}>{filters.map(f=><option key={f}>{f}</option>)}</select></label>
      <label>{t("compare-category")}<select aria-label={t("compare-category")} value={state.query.class ?? ""} onChange={e=>useComparison.setState({query:{...state.query,class:(e.target.value || undefined) as typeof state.query.class},selected:null})}>
        <option value="">{t("compare-all")}</option>{(changed ? ["changed","notChanged"] : CATEGORIES).map(c=><option key={c} value={c}>{c}</option>)}</select></label>
      {!changed && <label>{t("compare-source")}<select aria-label={t("compare-source")} value={state.query.side} onChange={e=>{setComparisonSide(e.target.value as "Current"|"Previous");}}><option>Current</option><option>Previous</option></select></label>}
      {!changed && <label>{t("compare-search")}<input type="search" value={state.query.search ?? ""} onChange={e=>useComparison.setState({query:{...state.query,search:e.target.value},selected:null})} /></label>}
      <button type="button" disabled={!page || exporting} onClick={()=>{setExporting(true);void downloadComparison(state.query,"comparison.csv").catch(setError).finally(()=>setExporting(false));}}>{t("compare-export")}</button>
    </div>
    {error != null && <p role="alert">{describeError(t, toCmdError(error)).text}</p>}
    {!page && error == null && <p role="status">{t("compare-loading")}</p>}
    {page?.contentAvailable===false && <p role="status">{t("compare-content-unavailable")}</p>}
    {page && <><p role="status">{t("compare-total",{count:page.total})}</p><div className="os-compare-scroll"><table aria-label={t("compare-table")}><thead><tr>{page.columns.map(c=><th key={c}>{c}</th>)}</tr></thead><tbody>{page.cells.map((row,i)=><tr key={page.rowKeys[i] ?? i} aria-selected={state.selected===page.rowKeys[i]}>{row.map((value,n)=><td key={n}>{n===0 ? <button type="button" onClick={()=>selectComparisonUrl(page.rowKeys[i]!,page.side,page.partnerKeys?.[i] ?? null)}>{String(value ?? "")}</button> : String(value ?? "")}</td>)}</tr>)}</tbody></table></div>
      <Button isDisabled={offset===0} onPress={()=>setOffset(n=>Math.max(0,n-100))}>{t("compare-page-previous")}</Button><Button isDisabled={offset+page.cells.length>=page.total} onPress={()=>setOffset(n=>n+100)}>{t("compare-page-next")}</Button>
      {page.total===0 && <p>{t("compare-empty")}</p>}
    </>}
  </section>;
}
