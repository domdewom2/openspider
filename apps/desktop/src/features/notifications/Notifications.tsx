import { useEffect, useRef, useState } from "react";
import { getT, i18n, useLanguageStore, useT } from "../../i18n";
import { openSettingsWindow } from "../../app/windows";
import { auth } from "../../ipc/auth";
import { config } from "../../ipc/config";
import { crawl } from "../../ipc/crawl";
import { diag } from "../../ipc/diag";
import type { Notice } from "../../ipc/gen/Notice";
import { isTauri, sendDesktopNotification, copyDesktopText } from "../../ipc/transport/tauri";
import { usePrefs } from "../../state/prefs";
import { useSession } from "../../state/session";
import { Button } from "../../ui/primitives/Button";
import { Dialog } from "../../ui/primitives/Dialog";
import { Banner, ToastRegion } from "../../ui/primitives/Feedback";
import { normalizeInput, useHeader } from "../header/model";
import { reportError } from "./controller";
import "./notifications.css";

export async function notifyCompletion(title: string, body: string): Promise<void> {
  if (document.visibilityState === "visible" || !usePrefs.getState().notifications) return;
  if (isTauri()) await sendDesktopNotification(title, body);
  else if (typeof Notification !== "undefined" && Notification.permission === "granted") new Notification(title, { body });
}

const completion = (n: Notice) => n.code === "crawl-completed" || n.code === "analysis-completed";
const pacing = (n: Notice) => n.code === "crawl-throttled" || n.code === "crawl-throttle-cleared";

/** Serial dialogs, immediate errors, and rare live announcements; numeric ticks never enter the live region. */
export function Notifications() {
  const t = useT();
  const lang = useLanguageStore((s) => s.lang);
  const notices = useSession((s) => s.notices);
  const stage = useSession((s) => s.crawl?.memoryStage);
  const seen = useRef(new WeakSet<Notice>());
  const [authSeen] = useState(() => new Set<string>());
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [httpConfirmed, setHttpConfirmed] = useState(false);
  const [copyResult, setCopyResult] = useState("");
  const current = notices.find((n) => !completion(n) && !pacing(n));
  const host = String(current?.args.host ?? "");
  const realm = String(current?.args.realm ?? "");
  const isAuth = current?.code === "auth-required";
  const authKey = `${useSession.getState().crawl?.crawlId ?? ""}:${host}:${realm}`;
  const secureHost = normalizeInput(host);
  const needsHttpConsent = secureHost?.startsWith("http:") === true;
  let message = "";
  if (current) {
    const noticeKey = `notice-${current.code === "low-disk" ? "disk-low" : current.code}`;
    if (i18n.has(lang, noticeKey)) message = t(noticeKey, current.args);
    else if (i18n.has(lang, current.code)) message = t(current.code, current.args);
    else message = t("notify-error", { code: String(current.args.errorCode ?? current.code) });
  }

  useEffect(() => {
    for (const n of notices) {
      if (seen.current.has(n)) continue;
      seen.current.add(n);
      if (completion(n)) {
        const tr = getT();
        void notifyCompletion(tr(`notice-${n.code}`), tr("notify-completed-count", { count: Number(n.args.crawled ?? useSession.getState().crawl?.crawled ?? 0) })).catch(reportError);
        useSession.getState().dismissNotice(n);
      } else if (pacing(n)) useSession.getState().dismissNotice(n);
    }
  }, [notices]);

  useEffect(() => { setUsername(""); setPassword(""); setHttpConfirmed(false); setCopyResult(""); }, [current]);
  useEffect(() => {
    if (!current || !isAuth) return;
    if (authSeen.has(authKey)) useSession.getState().dismissNotice(current);
    else authSeen.add(authKey);
  }, [current, isAuth, authKey, authSeen]);

  async function action(fn: () => Promise<unknown>, close = false) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try { await fn(); if (close && current) useSession.getState().dismissNotice(current); }
    catch (e) { reportError(e); }
    finally { busyRef.current = false; setBusy(false); }
  }
  function dismiss() { if (current && !busyRef.current) useSession.getState().dismissNotice(current); }
  function cancelDialog() {
    if (isAuth) { setPassword(""); void action(() => crawl.resume(), true); }
    else dismiss();
  }
  async function submitAuth() {
    if (!secureHost || needsHttpConsent && !httpConfirmed) return;
    const origin = new URL(secureHost).origin;
    // Secret only crosses the IPC boundary to the core's encrypted profile store, never config/UI persistence.
    const profile = await auth.saveProfile({ profile: { name: `basic:${origin}:${realm}`, kind: "basic", host: origin, username, secret: password } });
    setPassword("");
    const dto = await config.get();
    const cfg = asObject(dto.config);
    const authCfg = asObject(cfg.auth);
    const refs = Array.isArray(authCfg.profile_refs) ? authCfg.profile_refs : [];
    await config.set({ ...dto, config: { ...cfg, auth: { ...authCfg, profile_refs: [...new Set([...refs, profile.name])] } } as never });
    await crawl.resume();
  }
  const httpsFailure = current && ["start-connection", "start-dns", "err-start-connection"].includes(current.code) && String(current.args.startUrl ?? "").startsWith("https:");
  return <>
    {stage === "paused" && <div className="os-memory-banner"><Banner kind="error"><strong>{t("notify-memory-paused")}</strong>
      {[["speed.threads", "notify-reduce-threads"], ["rendering.mode", "notify-disable-rendering"], ["storage.store_html", "notify-disable-html"]].map(([anchor, label]) => <Button key={anchor} onPress={() => void openSettingsWindow(anchor)}>{t(label!)}</Button>)}
    </Banner></div>}
    <ToastRegion label={t("notify-title")} />
    <Dialog title={t(isAuth ? "notify-auth-title" : "notify-title")} isOpen={!!current} isDismissable={!busy} onOpenChange={(open) => { if (!open) cancelDialog(); }}>
      <p>{message}</p>
      {current?.args.detail != null && <details><summary>{t("notify-details")}</summary><pre>{String(current.args.detail)}</pre></details>}
      {current?.args.status != null && <p>{t("notify-status", { status: String(current.args.status) })}</p>}
      {current?.args.path != null && <p>{String(current.args.path)} · {t("notify-free", { free: String(current.args.freeBytes ?? "–") })}</p>}
      {current?.args.target != null && <p>{String(current.args.target)}</p>}
      {current?.code === "start-robots" && <Button onPress={() => void openSettingsWindow("robots.ignore_robots_txt")}>{t("notify-robots-setting")}</Button>}
      {httpsFailure && <Button isDisabled={busy} onPress={() => {
        const startUrl = String(current.args.startUrl);
        useHeader.setState({ url: startUrl.replace(/^https:/, "http:") }); dismiss();
      }}>{t("notify-try-http")}</Button>}
      {isAuth && <form onSubmit={(e) => { e.preventDefault(); void action(submitAuth, true); }}>
        <p>{t("notify-auth-host", { host, realm })}</p>
        <label>{t("notify-username")}<input autoComplete="username" value={username} onChange={(e) => setUsername(e.target.value)} /></label>
        <label>{t("notify-password")}<input type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} /></label>
        {needsHttpConsent && <label><input type="checkbox" checked={httpConfirmed} onChange={(e) => setHttpConfirmed(e.target.checked)} />{t("notify-http-consent")}</label>}
        <Button type="submit" isDisabled={busy || !secureHost || !username || !password || needsHttpConsent && !httpConfirmed}>{t("notify-auth-submit")}</Button>
      </form>}
      <div className="os-dialog-footer-actions">
        <Button isDisabled={busy} onPress={() => void action(() => diag.openLogs())}>{t("notify-log")}</Button>
        <Button isDisabled={busy} onPress={() => void action(async () => {
          const debug = await diag.debugInfo();
          const text = JSON.stringify(debug, null, 2);
          if (isTauri()) await copyDesktopText(text);
          else await navigator.clipboard.writeText(text);
          setCopyResult(t("notify-copied"));
        })}>{t("notify-copy")}</Button>
        <Button isDisabled={busy} onPress={cancelDialog}>{t(isAuth ? "header-cancel" : "notify-close")}</Button>
      </div>
      {copyResult && <p role="status">{copyResult}</p>}
    </Dialog>
  </>;
}

function asObject(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
}
