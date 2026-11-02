// M3-K01 (K3-01, OUT-260/OUT-265, CRAWL-077/078): Abschnitt „Chromium-Bereitstellung“ in Spider › Rendering.
// Zeigt, welches Chromium ein Crawl verwenden würde (konfigurierter Pfad → bereitgestellt → System), bietet den
// Download der festen Version „Chrome for Testing“ mit Fortschritt, Abbruch/Fortsetzen und Entfernen. Ist JavaScript
// gewählt und kein Chromium da, steht oben ein Hinweis mit dem Download-Knopf.
import { useCallback, useEffect, useRef, useState } from "react";
import { useFormat, useT, type TFn } from "../../../../i18n";
import { IpcError } from "../../../../ipc/errors";
import type { ChromiumStatus } from "../../../../ipc/gen/ChromiumStatus";
import type { ChromiumStatusArgs } from "../../../../ipc/gen/ChromiumStatusArgs";
import { render_provision } from "../../../../ipc/render_provision";
import { Banner, Button, ProgressBar } from "../../../../ui/primitives";
import { useConfig } from "../../frame/context";
import "./ChromiumProvision.css";

/** Abfrageintervall während eines Downloads. */
export const POLL_MS = 500;

/** Läuft gerade ein Download, eine Prüfung oder die Einrichtung? */
export function isBusy(status: ChromiumStatus): boolean {
  const job = status.job;
  return job === "verifying" || job === "installing" || (typeof job === "object" && "downloading" in job);
}

const KNOWN_ERRORS = new Set([
  "cancelled", "checksum", "network", "http", "size", "archive", "io", "in-use", "crawl-running", "busy", "unsupported",
]);

/** Text zu einem `err-chromium-…`-Schlüssel; Unbekanntes mit Detail. */
export function failureText(t: TFn, messageKey: string, detail?: string | null): string {
  const short = messageKey.replace(/^err-chromium-/, "");
  return KNOWN_ERRORS.has(short) ? t(`chromium-prov-err-${short}`) : t("chromium-prov-err-other", { detail: detail ?? messageKey });
}

type Action = (args: ChromiumStatusArgs) => Promise<ChromiumStatus>;

export function ChromiumProvision() {
  const t = useT();
  const fmt = useFormat();
  const { draft } = useConfig();
  const configured = (draft.get<string | null>("rendering.chromium_path") ?? "").trim() || null;
  const mode = draft.get<string>("rendering.mode");
  const [status, setStatus] = useState<ChromiumStatus | null>(null);
  const [actionError, setActionError] = useState<{ key: string; detail?: string | null } | null>(null);
  const live = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const next = await render_provision.status({ configuredPath: configured });
      if (live.current) setStatus(next);
    } catch {
      // Ohne Kern-Command (ältere Bridge, Test ohne Handler) bleibt der Abschnitt verborgen.
    }
  }, [configured]);

  useEffect(() => {
    live.current = true;
    void refresh();
    return () => { live.current = false; };
  }, [refresh]);

  const busy = status !== null && isBusy(status);
  useEffect(() => {
    if (!busy) return;
    const id = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(id);
  }, [busy, refresh]);

  const run = async (action: Action) => {
    setActionError(null);
    try {
      const next = await action({ configuredPath: configured });
      if (live.current) setStatus(next);
    } catch (e) {
      const err = e instanceof IpcError ? e.cmdError : null;
      if (live.current) setActionError({ key: err?.messageKey ?? "err-internal", detail: err?.detail ?? String(e) });
      void refresh();
    }
  };

  if (!status) return null;

  const job = status.job;
  const downloading = typeof job === "object" && "downloading" in job ? job.downloading : null;
  const failure = typeof job === "object" && "failed" in job ? job.failed : null;
  const size = status.downloadBytes ?? null;
  const installed = status.installed ?? null;
  const missingPath = status.active === "none" && (status.activePath ?? null) !== null;
  const canDownload = status.supported && !busy && installed === null;
  const downloadLabel = status.partialBytes > 0 && size !== null
    ? t("chromium-prov-resume", { done: fmt.bytes(status.partialBytes), total: fmt.bytes(size) })
    : t("chromium-prov-download", { size: size === null ? "?" : fmt.bytes(size) });
  const download = <Button variant="primary" onPress={() => void run(render_provision.download)}>{downloadLabel}</Button>;

  return (
    <section className="os-chromium-provision" aria-labelledby="chromium-prov-title" data-testid="chromium-provision">
      <h3 className="os-cfg-section" id="chromium-prov-title">{t("chromium-prov-title")}</h3>
      {mode === "javascript" && status.active === "none" && (
        <div data-testid="chromium-prov-js-missing">
          <Banner kind="error" action={!missingPath && canDownload ? download : undefined}>
            {t(missingPath ? "chromium-prov-js-missing-path" : "chromium-prov-js-missing")}
          </Banner>
        </div>
      )}
      <p className="os-hint">{t("chromium-prov-intro", { version: status.version })}</p>
      <p data-testid="chromium-prov-active">
        {status.active === "provisioned"
          ? t("chromium-prov-active-provisioned", { version: installed ?? status.version })
          : t(missingPath ? "chromium-prov-active-missing" : `chromium-prov-active-${status.active}`)}
        {status.activePath && <><br /><span className="os-cfg-path">{t("chromium-prov-path", { path: status.activePath })}</span></>}
      </p>
      {installed !== null && (
        <p data-testid="chromium-prov-installed">
          {t("chromium-prov-installed", { version: installed })}
          {installed !== status.version && <> {t("chromium-prov-outdated", { version: status.version })}</>}
        </p>
      )}
      {!status.supported && installed === null && <p className="os-hint" role="note">{t("chromium-prov-unsupported")}</p>}
      {downloading && (
        <div data-testid="chromium-prov-progress">
          <ProgressBar label={t("chromium-prov-progress")} value={downloading.done} max={Math.max(downloading.total, 1)} />
          <p role="status">{t("chromium-prov-downloading", { done: fmt.bytes(downloading.done), total: fmt.bytes(downloading.total) })}</p>
        </div>
      )}
      {(job === "verifying" || job === "installing") && (
        <div data-testid="chromium-prov-progress">
          <ProgressBar label={t("chromium-prov-progress")} />
          <p role="status">{t(job === "verifying" ? "chromium-prov-verifying" : "chromium-prov-installing")}</p>
        </div>
      )}
      {failure && !busy && (
        <div data-testid="chromium-prov-failure">
          <Banner kind={failure.messageKey === "err-chromium-cancelled" ? "info" : "error"}>
            {failureText(t, failure.messageKey, failure.detail)}
          </Banner>
        </div>
      )}
      {actionError && (
        <div data-testid="chromium-prov-action-error">
          <Banner kind="error">{failureText(t, actionError.key, actionError.detail)}</Banner>
        </div>
      )}
      <div className="os-chromium-provision-actions">
        {canDownload && download}
        {busy && job !== "installing" && (
          <Button onPress={() => void run(render_provision.cancel)}>{t("chromium-prov-cancel")}</Button>
        )}
        {!busy && installed !== null && status.supported && installed !== status.version && (
          <Button variant="primary" onPress={() => void run(render_provision.download)}>{t("chromium-prov-update", { version: status.version })}</Button>
        )}
        {!busy && installed === null && status.partialBytes > 0 && (
          <Button onPress={() => void run(render_provision.remove)}>{t("chromium-prov-discard")}</Button>
        )}
        {!busy && installed !== null && (
          <Button onPress={() => void run(render_provision.remove)}>{t("chromium-prov-remove")}</Button>
        )}
      </div>
    </section>
  );
}
