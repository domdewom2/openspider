import { useEffect } from "react";
import { useT } from "../../i18n";
import { update } from "../../ipc/update";
import { useSession } from "../../state/session";
import { registerAction } from "../../ui/actions";
import { Button, Dialog } from "../../ui/primitives";
import { accept, checkUpdates, downloadUpdate, installUpdate, useUpdate } from "./state";

export function UpdateHost() {
  const t = useT();
  const { state, release, open, queued } = useUpdate();
  const running = useSession(s => s.crawl?.phase === "running" || s.analysis?.running === true);
  useEffect(() => registerAction("help.updates", () => { void checkUpdates(); }), []);
  useEffect(() => {
    let live = true;
    let busy = false;
    const refresh = async () => {
      if (busy) return;
      busy = true;
      try {
        const state = await update.status();
        if (live) {
          accept(state);
          if (typeof state === "object" && "available" in state) {
            useUpdate.setState({ open: true });
            await downloadUpdate();
          }
        }
      } catch { /* Startup checks remain silent when the transport is unavailable. */ }
      finally { busy = false; }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 1000);
    return () => { live = false; clearInterval(timer); };
  }, []);
  const object = typeof state === "object" ? state : null;
  const ready = object && "ready" in object;
  const downloading = object && "downloading" in object ? object.downloading : null;
  return <Dialog title={t("update-title")} isOpen={open} onOpenChange={open => useUpdate.setState({ open })}>
    <div aria-live="polite">
      {state === "checking" && <p>{t("update-checking")}</p>}
      {state === "upToDate" && <p>{t("update-current")}</p>}
      {object && "disabled" in object && <p>{t(object.disabled === "environment" ? "update-environment" : "update-disabled")}</p>}
      {object && "failed" in object && <p role="alert">{t("update-failed")}</p>}
      {release && <><h3>{t("update-version", { version: release.version })}</h3><h4>{t("update-notes")}</h4><p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{release.notes || t("update-no-notes")}</p></>}
      {downloading && <><p>{t("update-downloading", { done: downloading.done, total: downloading.total ?? "?" })}</p><progress aria-label={t("update-progress")} value={downloading.total ? downloading.done : undefined} max={downloading.total ?? undefined} /></>}
      {queued && <p role="status">{t(queued === "onQuit" ? "update-queued-quit" : "update-queued-crawl")}</p>}
    </div>
    <div className="os-dialog-footer-actions">
      {object && ("available" in object || "failed" in object && release) && <Button onPress={() => void downloadUpdate()}>{t("update-download")}</Button>}
      {ready && !queued && (running ? <Button onPress={() => void installUpdate("afterCrawl")}>{t("update-after-crawl")}</Button> : <>
        <Button onPress={() => void installUpdate("onQuit")}>{t("update-on-quit")}</Button>
        <Button variant="primary" onPress={() => void installUpdate("now")}>{t("update-now")}</Button>
      </>)}
      <Button onPress={() => useUpdate.setState({ open: false })}>{t("update-close")}</Button>
    </div>
  </Dialog>;
}
