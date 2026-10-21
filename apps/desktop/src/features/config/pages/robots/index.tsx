// robots.txt (CRAWL-099 … 102): Modus, Verhalten bei Fehlern, Anzeige blockierter URLs, eigene Datei je Subdomain und Tester.
import { useState } from "react";
import { useT } from "../../../../i18n";
import { config } from "../../../../ipc/config";
import { toCmdError } from "../../../../ipc/errors";
import type { RobotsTestResult } from "../../../../ipc/gen/RobotsTestResult";
import { Banner, Button, TextField } from "../../../../ui/primitives";
import { messageText, useConfig, useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { getPath } from "../../frame/model";
import { FieldFrame } from "../../frame/fields";
import { resolveUa } from "../user-agent";
import type { FieldDef } from "../../frame/types";

const fields: FieldDef[] = [
  f("robots.mode", { kind: "radio", sectionKey: "config-sec-robots-mode" }),
  f("robots.respect_crawl_delay"),
  f("robots.unreachable_policy", { kind: "radio", sectionKey: "config-sec-robots-errors" }),
  f("robots.retries"),
  f("robots.retry_delay_s", { unitKey: "config-unit-s" }),
  f("robots.show_blocked_internal", { sectionKey: "config-sec-robots-show", enabledWhen: { path: "robots.mode", equals: "respect" } }),
  f("robots.show_blocked_external", { enabledWhen: { path: "robots.mode", equals: "respect" } }),
  f("robots.custom_files", { hidden: true, labelKey: "config-robots-custom" }),
];

/** Hostname ohne Schema, Pfad und Port-Standard (`https://Blog.Example.test/` → `blog.example.test`). */
export function normalizeHost(input: string): string {
  const s = input.trim().replace(/^[a-z][a-z0-9+.-]*:\/\//i, "").replace(/[/?#].*$/, "");
  return s.toLowerCase();
}

/** Host einer Test-URL (mit Port, wenn angegeben); `null` bei ungültiger Eingabe. */
export function hostOfUrl(url: string): string | null {
  try {
    return new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(url) ? url : `https://${url}`).host.toLowerCase();
  } catch {
    return null;
  }
}

function CustomRobots() {
  const t = useT();
  const { draft, runMode } = useConfig();
  const { value, set, lock } = useConfigValue<Record<string, string> | undefined>("robots.custom_files");
  const files = value ?? {};
  const hosts = Object.keys(files).sort();
  const [selected, setSelected] = useState<string | null>(hosts[0] ?? null);
  const [newHost, setNewHost] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [testUrl, setTestUrl] = useState("");
  const [result, setResult] = useState<{ ok: true; value: RobotsTestResult; text: string | null; line: number | null } | { ok: false; text: string } | null>(null);
  const current = selected !== null && selected in files ? selected : (hosts[0] ?? null);
  const disabled = lock !== null || runMode === "foreign";

  const add = () => {
    const host = normalizeHost(newHost);
    if (host === "" || host in files) return;
    set({ ...files, [host]: "" });
    setSelected(host);
    setNewHost("");
  };
  const remove = (host: string) => {
    const { [host]: _removed, ...rest } = files;
    void _removed;
    set(rest);
    setSelected(Object.keys(rest).sort()[0] ?? null);
  };
  const loadLive = async (host: string) => {
    setLoading(true);
    setLoadError(null);
    try {
      const body = await config.loadRobots({ host });
      set({ ...files, [host]: body });
    } catch (e) {
      const err = toCmdError(e);
      setLoadError(messageText(t, err.messageKey, err.args, err.detail));
    } finally {
      setLoading(false);
    }
  };
  const runTest = async () => {
    const host = hostOfUrl(testUrl);
    if (!host) {
      setResult({ ok: false, text: t("config-robots-test-url-invalid") });
      return;
    }
    // Der Entwurf der eigenen Datei gilt sofort, auch ohne Speichern.
    const own = host in files ? files[host] : undefined;
    try {
      // CRAWL-102: the draft user agent applies at once, like the draft of the file.
      const userAgent = resolveUa(getPath(draft.cfg, "user_agent") as Parameters<typeof resolveUa>[0], null).token;
      const r = await config.testRobots({ url: testUrl, content: own ?? null, subdomain: host, userAgent });
      const text = own !== undefined && r.matchedLine ? (own.split(/\r?\n/)[r.matchedLine - 1] ?? null) : null;
      setResult({ ok: true, value: r, text, line: r.matchedLine ?? null });
    } catch (e) {
      const err = toCmdError(e);
      setResult({ ok: false, text: messageText(t, err.messageKey, err.args, err.detail) });
    }
  };

  return (
    <fieldset className="os-cfg-group" data-testid="robots-custom">
      <legend>{t("config-robots-custom")}</legend>
      <p className="os-hint">{t("config-robots-custom-hint")}</p>
      <FieldFrame path="robots.custom_files">
        <div className="os-cfg-inline">
          <TextField label={t("config-robots-new-host")} value={newHost} onChange={setNewHost} isDisabled={disabled} placeholder="www.example.test" className="os-cfg-input" />
          <Button variant="secondary" onPress={add} isDisabled={disabled || normalizeHost(newHost) === ""}>
            {t("config-robots-add")}
          </Button>
        </div>
        {hosts.length === 0 ? (
          <p className="os-hint">{t("config-robots-none")}</p>
        ) : (
          <div className="os-cfg-robots-editor">
            <ul className="os-cfg-hosts" aria-label={t("config-robots-hosts")}>
              {hosts.map((h) => (
                <li key={h}>
                  <button type="button" className="os-cfg-link" aria-pressed={h === current} onClick={() => setSelected(h)}>
                    {h}
                  </button>
                </li>
              ))}
            </ul>
            {current !== null && (
              <div className="os-cfg-robots-file">
                <label className="os-label" htmlFor="os-cfg-robots-text">
                  {t("config-robots-file", { host: current })}
                </label>
                <textarea
                  id="os-cfg-robots-text"
                  className="os-input os-cfg-textarea os-mono"
                  rows={10}
                  spellCheck={false}
                  value={files[current] ?? ""}
                  disabled={disabled}
                  onChange={(e) => set({ ...files, [current]: e.target.value })}
                />
                <div className="os-cfg-inline">
                  <Button variant="secondary" onPress={() => void loadLive(current)} isDisabled={disabled || loading}>
                    {t("config-robots-load-live")}
                  </Button>
                  <Button variant="secondary" onPress={() => remove(current)} isDisabled={disabled}>
                    {t("config-robots-remove")}
                  </Button>
                </div>
                {loadError && <Banner kind="error">{loadError}</Banner>}
              </div>
            )}
          </div>
        )}
      </FieldFrame>
      <div className="os-cfg-tester" data-testid="robots-tester">
        <h3 className="os-cfg-section">{t("config-robots-tester")}</h3>
        <div className="os-cfg-inline">
          <TextField label={t("config-robots-test-url")} value={testUrl} onChange={setTestUrl} placeholder="https://www.example.test/blog/" className="os-cfg-input os-cfg-wide" />
          <Button variant="primary" onPress={() => void runTest()} isDisabled={testUrl.trim() === ""}>
            {t("config-robots-test")}
          </Button>
        </div>
        {result && result.ok && (
          <div role="status" data-testid="robots-result" data-allowed={result.value.allowed ? "true" : "false"}>
            <strong>{result.value.allowed ? t("config-robots-allowed") : t("config-robots-blocked")}</strong>
            {" · "}
            {t("config-robots-ua", { ua: result.value.userAgent })}
            {result.line !== null && (
              <p>
                {t("config-robots-line", { line: result.line })}
                {result.text !== null && <mark className="os-cfg-mark">{result.text}</mark>}
              </p>
            )}
          </div>
        )}
        {result && !result.ok && <Banner kind="error">{result.text}</Banner>}
      </div>
    </fieldset>
  );
}

export const page = definePage({
  id: "robots",
  order: 70,
  fields,
  anchors: ["robots"],
  After: CustomRobots,
});
