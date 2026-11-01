import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "../../../ui/primitives/Button";
import { ProgressBar } from "../../../ui/primitives/Feedback";
import { useT } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import { useSession } from "../../../state/session";

/**
 * UI-141/UI-145: while the core counts the filters of a crawl for the first time, Overview and Issues
 * show its progress (tabs counted) and offer cancelling it (button or Esc). Shown only after 300 ms, so
 * short counts do not flicker. A cancelled count resumes from the stored tabs on the next request.
 */
export function CountProgress() {
  const t = useT();
  const crawl = useSession(s => s.crawl?.crawlId);
  const status = useQuery({ queryKey: ["sidebar", "count", crawl], queryFn: ({ signal }) => sidebar.countStatus(signal), enabled: !!crawl, refetchInterval: 250, gcTime: 0, retry: false });
  const running = !!status.data?.running;
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    setVisible(false);
    if (!running) return;
    const timer = setTimeout(() => setVisible(true), 300);
    return () => clearTimeout(timer);
  }, [running]);
  if (!status.data || !running || !visible) return null;
  const { done, total } = status.data;
  const cancel = () => void sidebar.countCancel().then(() => status.refetch()).catch(() => undefined);
  // Esc cancels only with focus in the hint: the window-wide Esc is the global search.clear shortcut
  // (captured, TabControls.tsx), and a second global Esc consumer would cancel two things at once.
  // The cancel button is focusable, so Esc works after Tab/click into the hint.
  return <div className="os-sidebar-counting" role="status" onKeyDown={e => { if (e.key === "Escape") { e.preventDefault(); cancel(); } }}>
    <ProgressBar label={t("sidebar-counting-label")} value={total ? done : null} max={total || 100} />
    <span>{total ? t("sidebar-counting", { done, total }) : t("sidebar-counting-start")}</span>
    <Button onPress={cancel}>{t("sidebar-counting-cancel")}</Button>
  </div>;
}
