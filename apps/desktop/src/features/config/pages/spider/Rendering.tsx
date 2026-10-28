// Spider › Rendering (CRAWL-077): Modus Text/JavaScript, Chromium, Browser-Tabs, Screenshots und Gerät.
// Der Moduswechsel gehört zu den Feldern, die nur ein neuer Crawl ändern darf (CRAWL-126): während „Läuft“ und „Pausiert“
// ist der Umschalter gesperrt, mit Hinweis. Chromium-Fehler des Kerns erscheinen als Meldung mit Pfad-/Download-Option.
import { useT } from "../../../../i18n";
import { Banner, Button } from "../../../../ui/primitives";
import { issueText, useConfig } from "../../frame/context";
import { FieldList, PageHeading } from "../../frame/fields";
import { f } from "../../frame/definePage";
import { focusAnchor } from "../../frame/focus";
import { pageTitleKey, type FieldDef } from "../../frame/types";
import type { ValidationIssue } from "../../../../ipc/gen/ValidationIssue";

export const CHROMIUM_DOWNLOAD_URL = "https://www.chromium.org/getting-involved/download-chromium/";

export const renderingFields: FieldDef[] = [
  f("rendering.mode", { kind: "radio", sectionKey: "config-sec-render-mode" }),
  f("rendering.chromium_path", { kind: "text", hintKey: "config-hint-chromium-path" }),
  f("rendering.ajax_timeout_s", { unitKey: "config-unit-s" }),
  f("rendering.page_timeout_s", { unitKey: "config-unit-s" }),
  f("rendering.tabs", { nullLabelKey: "config-auto" }),
  f("rendering.js_error_reporting"),
  f("rendering.restart_after_pages", { sectionKey: "config-sec-restart" }),
  f("rendering.restart_memory_mb", { unitKey: "config-unit-mb" }),
  f("rendering.screenshots.enabled", { sectionKey: "config-sec-screenshots" }),
  f("rendering.screenshots.format", { kind: "radio" }),
  f("rendering.screenshots.jpeg_quality"),
  f("rendering.screenshots.wait_ms", { unitKey: "config-unit-ms" }),
  f("rendering.screenshots.fit_to_content"),
  f("rendering.screenshots.width", { nullLabelKey: "config-viewport-width", unitKey: "config-unit-px" }),
  f("rendering.screenshots.height", { nullLabelKey: "config-viewport-height", unitKey: "config-unit-px" }),
  f("rendering.viewport.preset", { kind: "text", sectionKey: "config-sec-viewport", hintKey: "config-hint-viewport-preset" }),
  f("rendering.viewport.width", { unitKey: "config-unit-px" }),
  f("rendering.viewport.height", { unitKey: "config-unit-px" }),
  f("rendering.viewport.scale_factor"),
  f("rendering.viewport.mobile"),
  f("rendering.viewport.touch"),
];

/** Meldungen des Kerns zu Chromium (Pfad `rendering.chromium_path` oder Chromium im Schlüssel/Grund). */
export function renderingProblems(issues: readonly ValidationIssue[]): ValidationIssue[] {
  return issues.filter((i) => !i.warning && (i.path === "rendering.chromium_path" || /chromium/i.test(`${i.messageKey} ${String(i.args.reason ?? "")}`)));
}

export function Rendering() {
  const t = useT();
  const { draft, runMode } = useConfig();
  const problems = renderingProblems(draft.issues);
  const mode = draft.get<string>("rendering.mode");
  return (
    <section className="os-cfg-page" aria-label={t(pageTitleKey("spider-rendering"))} data-page-id="spider-rendering">
      <PageHeading titleKey={pageTitleKey("spider-rendering")} />
      {runMode !== "free" && runMode !== "foreign" && (
        <p className="os-hint" role="note" data-testid="rendering-locked-note">
          {t("config-rendering-locked")}
        </p>
      )}
      {/* A02-21: say why the mode cannot be chosen and what to do – new crawl, Chromium (not bundled before M3, K2-09). */}
      {runMode !== "free" && runMode !== "foreign" && <p className="os-hint" role="note" data-testid="rendering-locked-how">{t("config-rendering-locked-how")}</p>}
      {mode === "javascript" && problems.length === 0 && <p className="os-hint">{t("config-rendering-js-note")}</p>}
      {mode !== "javascript" && <p className="os-hint" role="note" data-testid="rendering-chromium-note">{t("config-rendering-chromium-note")}</p>}
      {problems.map((p, n) => (
        <div key={n} data-testid="rendering-problem">
          <Banner
            kind="error"
            action={
              <Button variant="secondary" onPress={() => focusAnchor(document.querySelector(".os-config-content"), "rendering.chromium_path")}>
                {t("config-chromium-set-path")}
              </Button>
            }
          >
            {t("config-chromium-problem", { reason: issueText(t, p) })}{" "}
            <a href={CHROMIUM_DOWNLOAD_URL} target="_blank" rel="noopener noreferrer">
              {t("config-chromium-download")}
            </a>
          </Banner>
        </div>
      ))}
      <FieldList fields={renderingFields} />
    </section>
  );
}
