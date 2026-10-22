import { MockCore } from "../../../../ipc/transport/mock";
import type { CompareSession } from "../../../../ipc/gen/CompareSession";
import type { CrawlSummaryDto } from "../../../../ipc/gen/CrawlSummaryDto";
import type { Snapshot, ComparisonQuery } from "../../model";
import data from "./data.json";
export { data };
const summary=(id:string,date:string):CrawlSummaryDto=>({crawlId:id,name:id,file:`${id}.ospider`,phase:"completed",mode:"spider",startedAt:date,urls:4,fileBytes:100,schemaVersion:3,owner:{kind:"app",live:false,controllable:false}});
export const previous=summary("previous","2026-10-01T00:00:00Z"),current=summary("current","2026-10-02T00:00:00Z");
export const session:CompareSession={current,previous,mapping:[],elements:["page_titles","word_count"],contentThresholdPct:10,version:1};
export const snapshot:Snapshot={overview:data.overview as Snapshot["overview"],issues:data.issues as Snapshot["issues"],notes:[]};
export function comparisonFixture(){
  const core=new MockCore();
  core.handlers.set("crawls.list",()=>[previous,current].map(crawl=>({crawl,locked:false,canResume:false})));
  core.handlers.set("compare.open",()=>session);
  core.handlers.set("compare.close",()=>undefined);
  core.handlers.set("compare.swap",()=>({...session,current:previous,previous:current,version:2}));
  core.handlers.set("compare.testMapping",args=>{const a=args as {input:string;rules:{pattern:string;replacement:string}[]};return {output:a.input,steps:a.rules.map((r,index)=>({index,matched:false,before:a.input,after:a.input,error:r.pattern==="["?"invalid regular expression at position 1":null}))};});
  core.handlers.set("sidebar.overview",()=>({summary:[],tabs:[],comparison:snapshot}));
  core.handlers.set("view.rows",args=>{
    const a=args as {comparison:ComparisonQuery;offset:number;version:number};const q=a.comparison;
    let cells:(string|number|null)[][]=[["https://site.test/start","Unchanged"]];
    let columns=["Address","Comparison Status"];const isLink=q.urlId!=null;
    if(isLink){
      columns=data.linkColumns;
      const source=q.source==="Auto"?q.side:q.source;
      // Fixture transport only: production classification runs in Rust and is checked through Playwright.
      cells=q.urlId===99 ? data.currentLinks.slice(0,1) : q.change==="Removed" ? data.previousLinks.slice(0,1) : q.change==="Added" ? data.currentLinks.slice(0,1) : (source==="Previous" ? data.previousLinks : data.currentLinks).filter(row=>!q.change || q.change==="All" || row[4]===q.change);
    }
    return {viewId:"fixture-comparison",version:a.version,offset:a.offset,rowKeys:cells.map((_,i)=>i+1),columns,cells,partnerKeys:cells.map((_,i)=>101+i),total:cells.length,side:q.side,filtersEnabled:q.urlId!==99};
  });
  return {core};
}
