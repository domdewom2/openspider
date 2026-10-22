import { useState } from "react";
import { useT } from "../../i18n";
import { CATEGORIES, navigateComparison, type FilterComparison, useComparison } from "./model";
export function ComparisonNumbers({ issues = false }: { issues?: boolean }) {
  const t = useT(); const state = useComparison(); const [descending, setDescending] = useState(true);
  const data = state.snapshot?.[issues ? "issues" : "overview"] ?? [];
  const rows = issues ? [...data].sort((a,b) => {
    const missing = Number(a.change==null)-Number(b.change==null);
    const delta = a.change!=null && b.change!=null ? (descending ? b.change-a.change : a.change-b.change) : 0;
    return missing || delta || `${a.tab}/${a.filter}`.localeCompare(`${b.tab}/${b.filter}`,"en");
  }) : data;
  const click = (r: FilterComparison, index: number) => {
    navigateComparison(r.tab,r.filter,index >= 3 ? CATEGORIES[index-3] : undefined,index === 1 ? "Previous" : "Current");
    if(r.tab==="Change Detection")useComparison.setState(s=>({query:{...s.query,class:"changed",side:"Current"}}));
  };
  return <div className="os-compare-numbers">
    {state.snapshot?.notes.map((note,i) => <p role="status" key={i}>{t(`compare-note-${note.kind}`, { address: note.address ?? "", winner: note.winner ?? "", rule: note.rule ?? 0 })}</p>)}
    <table aria-label={t(issues ? "compare-issues" : "compare-overview")}><thead><tr><th>{t("compare-filter")}</th>
      {["Current","Previous","Change",...CATEGORIES].map(name => <th key={name}>{issues && name === "Change" ? <button type="button" onClick={() => setDescending(v => !v)}>{name} {descending ? "↓" : "↑"}</button> : name}</th>)}
    </tr></thead><tbody>{rows.map(row => <tr key={`${row.tab}/${row.filter}`}><th scope="row">{row.tab}: {row.label}</th>
      {[row.current,row.previous,row.change,...(row.classes ?? [null,null,null,null])].map((count,i) => <td key={i}>{count == null ? "–" : i === 2 ? count : <button type="button" onClick={() => click(row,i)}>{count}</button>}</td>)}
    </tr>)}</tbody></table>
  </div>;
}
