// Testseite für M2-U13 (nur Vite-Dev-Server): Konfigurationsdialog mit Mock-Kern (`?transport=mock`) oder echter Bridge
// (`?bridge=<ws-url>`, `os-app-bridge`). Mit `?lang=de` startet die Seite deutsch.
import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { ConfigHost } from "../../src/features/config/frame/ConfigHost";
import { installConfigCore } from "../../src/features/config/frame/testing";
import { closeConfig, openConfig } from "../../src/features/config/frame/store";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import type { CrawlStatus } from "../../src/ipc/gen/CrawlStatus";
import { useSession } from "../../src/state/session";
import { runAction } from "../../src/ui/actions";
import "../../src/ui/ui.css";

const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage(new URLSearchParams(location.search).get("lang") ?? "en");

const core = window.__osMock;
const fake = core ? installConfigCore(core) : null;

const api = {
  open: (anchor?: string) => openConfig(anchor ? { anchor } : {}),
  close: closeConfig,
  action: (id: string) => runAction(id, { source: "menu" }),
  calls: () => core?.calls ?? [],
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
  /** Crawl-Zustand der Statusleiste nachbilden (Mock): Läuft, Pausiert oder fremd gestartet. */
  setCrawl(phase: CrawlStatus["phase"] | null, controllable = true) {
    if (fake) fake.mode = phase === "running" ? "running" : phase === null ? "free" : "paused";
    useSession.setState({
      crawl: phase === null ? null : ({ crawlId: "c1", phase, mode: "spider", crawled: 10, queued: 5, discovered: 15, percent: 0, rate: 0, avgRate: 0, elapsedS: 1, pausedS: 0, activeThreads: 1, owner: { kind: "app", live: true, controllable } } as unknown as CrawlStatus),
    });
  },
  /** Nächstes `config.set` scheitern lassen (Mock), z. B. Chromium fehlt. */
  failNextSet(error: { code: "InvalidInput"; messageKey: string; field?: string; detail?: string }) {
    if (fake) fake.failNextSet = error;
  },
  saved: () => fake?.config ?? null,
  profiles: () => (fake ? [...fake.profiles.keys()] : null),
  seedProfile(name: string, config: Record<string, unknown>) {
    fake?.profiles.set(name, config);
  },
  pauseCalls: () => fake?.pauseCalls ?? 0,
  /** Der Mock-Kern pausiert auf `crawl.pause` wie die Statusleiste. */
  pauseFollowsCommand() {
    core?.handlers.set("crawl.pause", () => {
      if (fake) fake.pauseCalls += 1;
      api.setCrawl("paused");
      return null;
    });
  },
  transport: runtime.transportName,
};
declare global {
  interface Window {
    cfgFixture: typeof api;
  }
}
window.cfgFixture = api;

createRoot(document.getElementById("root")!).render(
  <main>
    <button id="open-config" type="button" onClick={() => openConfig()}>
      Open configuration
    </button>
    <ConfigHost />
  </main>,
);
