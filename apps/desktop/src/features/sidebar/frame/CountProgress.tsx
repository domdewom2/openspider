import { useEffect, useState } from "react";
import { Button } from "../../../ui/primitives/Button";
import { ProgressBar } from "../../../ui/primitives/Feedback";
import { useT } from "../../../i18n";
import { op } from "../../../ipc/op";
import { useSession } from "../../../state/session";

/** Label key of the core's one-time Overview/Issues count (`sidebar.rs`, `COUNT_LABEL`). */
export const COUNT_OP = "sidebar-op-count";

/**
 * UI-141/UI-145: while the core counts the filters of a crawl for the first time, Overview and Issues
 * show its progress (tabs counted) and offer cancelling it (button or Esc). Shown only after 300 ms, so
 * short counts do not flicker. A cancelled count resumes from the stored tabs on the next request.
 */
export function CountProgress() {
  const t = useT();
  const status = useSession(s => Object.values(s.ops).find(o => o.labelKey === COUNT_OP && o.state === "running"));
  const running = !!status;
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    setVisible(false);
    if (!running) return;
    const timer = setTimeout(() => setVisible(true), 300);
    return () => clearTimeout(timer);
  }, [running]);
  if (!status || !visible) return null;
  const total = status.total ?? null;
  const cancel = () => void op.cancel({ opId: status.opId }).catch(() => undefined);
  return <div className="os-sidebar-counting" role="status" onKeyDown={e => { if (e.key === "Escape") { e.preventDefault(); cancel(); } }}>
    <ProgressBar label={t("sidebar-counting-label")} value={total ? status.done : null} max={total ?? 100} />
    <span>{total ? t("sidebar-counting", { done: status.done, total }) : t("sidebar-counting-start")}</span>
    <Button onPress={cancel}>{t("sidebar-counting-cancel")}</Button>
  </div>;
}
