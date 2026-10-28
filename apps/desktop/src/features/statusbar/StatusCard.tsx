import { useFormat, useT } from "../../i18n";
import { ProgressBar } from "../../ui/primitives/Feedback";
import { useStatusSnapshot } from "./Statusbar";
import "./statusbar.css";

/** Presentation of the existing throttled session snapshot; no timers or commands. */
export function StatusCard() {
  const t = useT();
  const f = useFormat();
  const { crawl: c } = useStatusSnapshot();
  const phase = c?.endReason === "stopped" ? t("n03-status-stopped") : t(`status-${c?.phase ?? "idle"}`);
  return <section className="os-status-card" aria-label={t("header-progress")} data-phase={c?.phase ?? "idle"}>
    <strong className="os-status-card-state"><span className="os-status-dot" aria-hidden="true" />{phase}</strong>
    <div className="os-status-card-progress"><ProgressBar label={t("header-progress")} value={c?.percent ?? 0} /><strong>{f.decimal(c?.percent ?? 0)} %</strong></div>
    <span>{t("u35-card-count", { done: f.integer(c?.crawled ?? 0), total: f.integer((c?.crawled ?? 0) + (c?.queued ?? 0)) })}</span>
    <span>{t("u35-card-rate", { value: f.decimal(c?.rate ?? 0) })}</span>
    <span>{t("u35-card-elapsed", { value: f.duration(c?.elapsedS ?? 0) })}</span>
    <span>{t("u35-card-eta", { value: c?.etaS == null ? "–" : f.duration(c.etaS) })}</span>
  </section>;
}
