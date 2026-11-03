// M3-K01 (K3-01): Testseite für den Abschnitt „Chromium-Bereitstellung“ (nur Vite-Dev-Server, `?transport=mock`).
// Der Mock-Kern bildet den Ablauf des Kerns nach: Download in Schritten je Statusabfrage, Prüfung, Einrichtung,
// Abbruch mit fortsetzbarem Teil-Download, Entfernen. Das echte Verhalten prüfen die Rust-Tests
// (`crates/os-render/tests/provision_*.rs`, `crates/os-app/tests/render_provision.rs`).
import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { ConfigHost } from "../../src/features/config/frame/ConfigHost";
import { openConfig } from "../../src/features/config/frame/store";
import { installConfigCore } from "../../src/features/config/frame/testing";
import { setLanguage } from "../../src/i18n";
import type { ChromiumStatus } from "../../src/ipc/gen/ChromiumStatus";
import "../../src/ui/ui.css";

const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage(new URLSearchParams(location.search).get("lang") ?? "en");

const core = window.__osMock;
if (!core) throw new Error("render-provision fixture needs ?transport=mock");
const fake = installConfigCore(core);

const MB = 1_000_000;
const TOTAL = 186 * MB;
const EXE = "/data/OpenSpider/browser/156.0.8078.80/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";
const base = (): ChromiumStatus => ({
  supported: true, version: "156.0.8078.80", downloadBytes: TOTAL, installed: null, installedPath: null,
  active: "none", activePath: null, partialBytes: 0, job: "idle",
});
let status = base();
let hold = false;

/** Ein Schritt des simulierten Downloads je Statusabfrage. */
function advance() {
  const job = status.job;
  if (typeof job === "object" && "downloading" in job) {
    // Angehalten: höchstens bis zur Hälfte, dann „stockt“ die Übertragung (wie der Mock `/stall` der Rust-Tests).
    if (hold && job.downloading.done >= TOTAL / 2) return;
    const done = Math.min(TOTAL, job.downloading.done + TOTAL / 4);
    status = { ...status, partialBytes: done, job: done >= TOTAL ? "verifying" : { downloading: { done, total: TOTAL } } };
  } else if (job === "verifying") {
    status = { ...status, job: "installing" };
  } else if (job === "installing") {
    status = { ...base(), installed: "156.0.8078.80", installedPath: EXE, active: "provisioned", activePath: EXE };
  }
}
const h = core.handlers;
h.set("chromium.status", () => { advance(); return structuredClone(status); });
h.set("chromium.download", () => {
  status = { ...status, job: { downloading: { done: status.partialBytes, total: TOTAL } } };
  return structuredClone(status);
});
h.set("chromium.cancel", () => {
  const job = status.job;
  if (typeof job === "object" && "downloading" in job) {
    status = { ...status, partialBytes: job.downloading.done, job: { failed: { messageKey: "err-chromium-cancelled", detail: "" } } };
  }
  return structuredClone(status);
});
h.set("chromium.remove", () => { status = base(); return structuredClone(status); });

const api = {
  open: (anchor = "rendering.mode") => openConfig({ anchor }),
  setMode(mode: "text" | "javascript") {
    const config = fake.config as unknown as { rendering: Record<string, unknown> };
    config.rendering = { ...config.rendering, mode };
  },
  /** Download bei der Hälfte anhalten bzw. freigeben (für den Abbruch). */
  hold(value: boolean) { hold = value; },
  calls: () => core.calls.filter((c) => c.cmd.startsWith("chromium.")).map((c) => c.cmd),
};
declare global {
  interface Window {
    provFixture: typeof api;
  }
}
window.provFixture = api;

createRoot(document.getElementById("root")!).render(
  <main>
    <ConfigHost />
  </main>,
);
