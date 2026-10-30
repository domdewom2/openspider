import { loginStatus } from "../config/pages/auth-forms/session";
import { useEffect, useState } from "react";
import { useFormat, useT } from "../../i18n";
import { useSession, type SessionState } from "../../state/session";
import { McpMenu } from "./McpMenu";
import { ProgressBar } from "../../ui/primitives/Feedback";
import { useHeader } from "../header/model";
import "./statusbar.css";

/** Latest snapshot, at most four paints/s. Notices are handled immediately by their own subscriber. */
export function useStatusSnapshot(): SessionState {
  const [snapshot, setSnapshot] = useState(useSession.getState);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const off = useSession.subscribe(() => {
      timer ??= setTimeout(() => { timer = undefined; setSnapshot(useSession.getState()); }, 250);
    });
    setSnapshot(useSession.getState());
    return () => { off(); clearTimeout(timer); };
  }, []);
  return snapshot;
}

export function Statusbar() {
  const t = useT();
  const f = useFormat();
  const { crawl: c, mcp, throttled, crawlDelays, analysis: storedAnalysis, analysisStarting, analysisResult } = useStatusSnapshot();
  const analysis = analysisStarting ? { steps: [], running: true, current: null, percent: 0 } : storedAnalysis;
  const [sessionLost, setSessionLost] = useState(false);
  useEffect(() => {
    let live = true; setSessionLost(false);
    if (!c) return;
    const refresh = () => void loginStatus().then((rows) => { if (live) setSessionLost(rows.some((r) => r.state === "lost")); }).catch(() => undefined);
    refresh(); const timer = setInterval(refresh, 1000);
    return () => { live = false; clearInterval(timer); };
  }, [c?.crawlId]);
  const chosenMode = useHeader((s) => s.mode);
  const phase = analysis?.running ? t("status-analysis") : c?.endReason === "stopped" ? t("n03-status-stopped") : t(`status-${c?.phase ?? "idle"}`);
  const unknown = "–";
  const current = analysis?.steps.find((s) => s.step === analysis.current);
  const index = current ? analysis!.steps.indexOf(current) + 1 : null;
  const memory = (bytes: number | null | undefined) => bytes == null ? unknown : f.bytes(bytes);
  return <div className="os-crawl-status" data-testid="crawl-status">
    <div className="os-status-scroll" role="group" aria-label={t("status-details")} tabIndex={0}>
    <span>{t("status-mode", { mode: t(`header-${c?.mode ?? chosenMode}`) })}</span>
    {sessionLost && <span role="status" className="os-status-warning">{t("form-login-session-lost")}</span>}
    <span data-testid="crawl-phase">{phase}{c?.endReason === "limit_reached" && ` (${t("status-limit")})`}</span>
    {Object.keys(throttled).length > 0 && <span className="os-status-warning">{t("status-throttled")}</span>}
    {c?.phase === "running" && Object.entries(crawlDelays).map(([origin, delay]) => <span key={origin} title={origin}>{t(delay.applied ? delay.effectiveSeconds < delay.seconds ? "n03-status-delay-capped" : "n03-status-delay-applied" : "n03-status-delay-ignored", { seconds: f.decimal(delay.seconds), effectiveSeconds: f.decimal(delay.effectiveSeconds) })}</span>)}
    <span>{t("status-rate", { value: f.decimal(c?.rate ?? 0) })}</span>
    <span>{t("status-average", { value: f.decimal(c?.avgRate ?? 0) })}</span>
    <span>{t("status-count", { done: f.integer(c?.crawled ?? 0), total: f.integer((c?.crawled ?? 0) + (c?.queued ?? 0)), percent: c?.percent ?? 0 })}</span>
    <span>{t("status-queued", { value: f.integer(c?.queued ?? 0) })}</span>
    <span>{t("status-eta", { value: c?.etaS == null ? unknown : f.duration(c.etaS) })}</span>
    <span>{t("status-threads", { value: c?.activeThreads ?? 0 })}</span>
    <span className={c?.memoryStage === "paused" ? "os-status-danger" : c?.memoryStage === "warning" ? "os-status-warning" : undefined}>
      {t("status-memory", { used: memory(c?.memoryUsedBytes), limit: memory(c?.memoryLimitBytes) })}
      {c?.memoryStage && c.memoryStage !== "normal" && ` (${t(`status-memory-${c.memoryStage}`)})`}
    </span>
    <span>{t("status-file", { value: memory(c?.fileBytes) })}</span>
    </div>
    <McpMenu>
      {mcp?.kind === "connected" ? t(mcp.clients === 1 ? "status-mcp-client" : "status-mcp-clients", { count: mcp.clients }) : mcp?.kind === "ready" ? t("status-mcp-ready", { port: mcp.port }) : t("status-mcp-off")}
    </McpMenu>
    <details><summary>{t("status-details")}</summary><div className="os-status-details">
      <span>{t("status-elapsed", { value: f.duration(c?.elapsedS ?? 0) })}</span>
      <span>{t("status-response", { value: c?.avgResponseTime == null ? unknown : f.decimal(c.avgResponseTime, 3) })}</span>
      <span>{t("status-end-reason", { value: c?.endReason ?? unknown })}</span>
    </div></details>
    {(analysis?.running || analysisResult) && <span role="status" data-testid="analysis-feedback">{t(analysis?.running ? "status-analysis" : `n03-analysis-${analysisResult}`)}</span>}
    {analysis?.running && <div className="os-analysis-progress"><span>{t(`status-step-${analysis.current}`, {}, { fallback: analysis.current ?? t("status-analysis") })}{index && ` ${index}/${analysis.steps.length}`} · {analysis.percent ?? unknown} %</span><ProgressBar label={t("status-analysis")} value={analysis.percent} /></div>}
  </div>;
}
