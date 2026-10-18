import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { AnalysisProgress } from "../analysis/AnalysisProgress";
import { SidebarFrame } from "./SidebarFrame";

/** Additive mount into the F04 landmark, until the layout package owns its composition. */
export function SidebarMount() {
  const [host, setHost] = useState<Element | null>(null);
  useEffect(() => setHost(document.querySelector(".os-app-sidebar")), []);
  const status = host?.parentElement?.querySelector(".os-app-status");
  return <>{host && createPortal(<SidebarFrame />, host)}{status && createPortal(<AnalysisProgress />, status)}</>;
}
