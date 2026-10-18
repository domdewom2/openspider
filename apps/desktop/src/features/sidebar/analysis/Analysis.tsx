import { useState } from "react";
import { analysis } from "../../../ipc/analysis";
import { op } from "../../../ipc/op";
import type { OpHandle } from "../../../ipc/gen/OpHandle";
import { useT } from "../../../i18n";
import { openSettingsWindow } from "../../../app/windows";
import { ProgressBar } from "../../../ui/primitives/Feedback";
import { useSession } from "../../../state/session";
import { useAnalysis } from "../frame/useSidebarQuery";
import { QueryFeedback } from "../frame/QueryFeedback";

export function useStartAnalysis() {
  const [error, setError] = useState<unknown>();
  const [handle, setHandle] = useState<OpHandle | null>(null);
  const [starting, setStarting] = useState(false);
  const operation = useSession(s => handle ? s.ops[handle.opId] : undefined);
  async function start(steps?: string[]) {
    setStarting(true);
    setError(undefined);
    try { setHandle(await analysis.start({ steps: steps ?? null })); } catch (e) { setError(e); } finally { setStarting(false); }
  }
  async function cancel() {
    if (!handle) return;
    try { await op.cancel({ opId: handle.opId }); } catch (e) { setError(e); }
  }
  const busy = starting || !!handle && (!operation || operation.state === "running");
  return { start, cancel, error: operation?.error ?? error, starting: busy, handle, operation };
}

export function Analysis({ controls }: { controls: ReturnType<typeof useStartAnalysis> }) {
  const t = useT();
  const query = useAnalysis();
  const crawl = useSession(s => s.crawl);
  const status = query.data;
  const stale = status?.steps.some(s => s.stale);
  const recrawl = status?.steps.some(s => s.state === "needsRecrawl");
  return <section aria-label={t("sidebar-analysis")} className="os-sidebar-analysis">
    <QueryFeedback error={query.error ?? controls.error} loading={!!crawl && query.isPending} retry={() => void query.refetch()} />
    {stale && <p role="status">{t("sidebar-analysis-stale")}</p>}
    {recrawl && <p role="status">{t("sidebar-html-required")} <button type="button" onClick={() => void openSettingsWindow("extraction.store_html")}>{t("sidebar-html-setting")}</button></p>}
    {status?.running && <div role="status"><p>{t("sidebar-analysis-running")}: {status.current ? t(`sidebar-step-${status.current}`, undefined, { fallback: status.current }) : t("sidebar-analysis")}</p><ProgressBar label={t("sidebar-analysis")} value={status.percent} />{controls.handle?.cancellable && <button type="button" onClick={() => void controls.cancel()}>{t("sidebar-cancel")}</button>}</div>}
    <button type="button" disabled={!crawl || status?.running || controls.starting} onClick={() => void controls.start()}>{t("sidebar-reanalyze")}</button>
    {status && <table className="os-sidebar-table"><thead><tr><th>{t("sidebar-analysis-step")}</th><th>{t("sidebar-analysis-status")}</th></tr></thead><tbody>{status.steps.map(step => <tr key={step.step} className={step.stale ? "os-sidebar-stale" : undefined}><th scope="row">{t(`sidebar-step-${step.step}`, undefined, { fallback: step.step })}</th><td>{t(`sidebar-step-state-${step.state}`)}{step.stale && ` · ${t("sidebar-filter-stale")}`}</td></tr>)}</tbody></table>}
  </section>;
}
