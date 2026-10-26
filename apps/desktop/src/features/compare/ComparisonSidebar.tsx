import { useState } from "react";
import { useT } from "../../i18n";
import { ComparisonNumbers } from "./ComparisonNumbers";
export function ComparisonSidebar() {
  const t=useT();const [issues,setIssues]=useState(false);
  return <section><div role="tablist" aria-label={t("sidebar-tabs")}><button type="button" role="tab" aria-selected={!issues} onClick={()=>setIssues(false)}>{t("sidebar-tab-overview")}</button><button type="button" role="tab" aria-selected={issues} onClick={()=>setIssues(true)}>{t("sidebar-tab-issues")}</button></div><ComparisonNumbers issues={issues} /></section>;
}
