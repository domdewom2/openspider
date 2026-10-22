import { downloadComparison } from "./export";
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { comparisonRows, LINK_CHANGES, type ComparisonPage, type Side, useComparison } from "./model";
export function ComparisonLinks({direction,urlId}:{direction:"inlinks"|"outlinks";urlId:number}) {
  const t=useT();const state=useComparison();const [source,setSource]=useState<"Auto"|Side>("Auto");const [change,setChange]=useState("All");const [page,setPage]=useState<ComparisonPage>();const [error,setError]=useState<unknown>();const [offset,setOffset]=useState(0);const [exporting,setExporting]=useState(false);
  useEffect(()=>{setOffset(0);},[source,change,urlId,state.query.side,state.revision]);
  useEffect(()=>{
    const controller=new AbortController();setPage(undefined);setError(undefined);
    void comparisonRows({tab:direction,filter:"All",side:state.query.side,urlId,source,change},offset,controller.signal).then(result=>{if(!controller.signal.aborted)setPage(result);}).catch(e=>{if(!controller.signal.aborted)setError(e);});
    return ()=>controller.abort();
  },[direction,urlId,source,change,offset,state.query.side,state.revision]);
  return <section aria-label={direction}><h3>{t(`detail-tab-${direction}`)}</h3><div className="os-compare-form">
    <label>{t("compare-source")}<select aria-label={t("compare-source")} value={source} onChange={e=>setSource(e.target.value as typeof source)}>{["Auto","Current","Previous"].map(v=><option key={v}>{v}</option>)}</select></label>
    <label>{t("compare-link-change")}<select aria-label={t("compare-link-change")} disabled={!page?.filtersEnabled} value={page?.filtersEnabled===false ? "All" : change} onChange={e=>setChange(e.target.value)}>{LINK_CHANGES.map(v=><option key={v} disabled={v!=="All" && !page?.filtersEnabled}>{v}</option>)}</select></label>
    <button type="button" disabled={!page || exporting} onClick={()=>{setExporting(true);void downloadComparison({tab:direction,filter:"All",side:state.query.side,urlId,source,change},`${direction}-comparison.csv`).catch(setError).finally(()=>setExporting(false));}}>{t("compare-export")}</button>
  </div>{error!=null && <p role="alert">{describeError(t, toCmdError(error)).text}</p>}{!page && !error && <p role="status">{t("compare-loading")}</p>}
    {page && <><p role="status">{t("compare-total",{count:page.total})}</p><div className="os-compare-scroll"><table><thead><tr>{page.columns.map(c=><th key={c}>{c}</th>)}</tr></thead><tbody>{page.cells.map((row,i)=><tr key={i}>{row.map((v,n)=><td key={n}>{String(v??"")}</td>)}</tr>)}</tbody></table></div>
      <button type="button" disabled={offset===0} onClick={()=>setOffset(n=>Math.max(0,n-100))}>{t("compare-page-previous")}</button><button type="button" disabled={offset+page.cells.length>=page.total} onClick={()=>setOffset(n=>n+100)}>{t("compare-page-next")}</button></>}
  </section>;
}
