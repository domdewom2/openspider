import { useState } from "react";
import { startAnalysis } from "./start";
import type { OpHandle } from "../../../ipc/gen/OpHandle";
import type { AnalysisStatus } from "../../../ipc/gen/AnalysisStatus";
import { useT } from "../../../i18n";
import { openConfig } from "../../config/frame/store";
import { useSession } from "../../../state/session";

export function useStartAnalysis() {
  const [error, setError] = useState<unknown>();
  const [handle, setHandle] = useState<OpHandle | null>(null);
  const [starting, setStarting] = useState(false);
  const operation = useSession(s => handle ? s.ops[handle.opId] : undefined);
  async function start() {
    setStarting(true);
    setError(undefined);
    try { setHandle(await startAnalysis({ steps: null })); } catch (e) { setError(e); } finally { setStarting(false); }
  }
  const busy = starting || !!handle && (!operation || operation.state === "running");
  return { start, error: operation?.error ?? error, starting: busy };
}

/** UI-080/UI-082: notices stay visible above every sidebar tab; U03 owns progress. */
export function AnalysisNotice({ status, controls }: { status?: AnalysisStatus; controls: ReturnType<typeof useStartAnalysis> }) {
  const t = useT();
  return <>
    {status?.steps.some(s => s.stale) && <p role="status">{t("sidebar-analysis-stale")} <button type="button" disabled={status.running || controls.starting} onClick={() => void controls.start()}>{t("sidebar-reanalyze")}</button></p>}
    {status?.steps.some(s => s.state === "needsRecrawl") && <p role="status">{t("sidebar-html-required")} <button type="button" onClick={() => openConfig({ anchor: "extraction.store_html" })}>{t("sidebar-html-setting")}</button></p>}
  </>;
}
