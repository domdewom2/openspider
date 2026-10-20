// Testseite für M2-U26 (nur Vite-Dev-Server): Integrationen-Host und Kopfleisten-Platz mit Mock-Kern oder echter Bridge.
// Mit `?transport=mock` antworten zustandsbehaftete Mock-Handler; mit `?bridge=<ws-url>` antwortet `os-app-bridge`.
import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import type { ApiConnectArgs } from "../../src/ipc/gen/ApiConnectArgs";
import type { ApiKind } from "../../src/ipc/gen/ApiKind";
import type { ApiRow } from "../../src/ipc/gen/ApiRow";
import type { ApiSetSettingsArgs } from "../../src/ipc/gen/ApiSetSettingsArgs";
import type { ApiSourceInfo } from "../../src/ipc/gen/ApiSourceInfo";
import type { CmdError } from "../../src/ipc/gen/CmdError";
import type { JsonValue } from "../../src/ipc/gen/serde_json/JsonValue";
import { ApiSourceGear, IntegrationsHeaderSlot, IntegrationsHost, openApiAccess, registerPropertyCatalog, type Ga4Account } from "../../src/features/integrations";
import { useSession } from "../../src/state/session";
import { runAction } from "../../src/ui/actions";
import fixture from "../../src/features/integrations/fixtures/integ-ui.json";
import "../../src/ui/ui.css";

const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage(new URLSearchParams(location.search).get("lang") ?? "en");

const core = window.__osMock;
let sources = structuredClone(fixture.sources) as ApiSourceInfo[];
const settings = structuredClone(fixture.settings) as Record<string, JsonValue>;
let rows: ApiRow[] = [];
let seq = 0;
const patch = (kind: ApiKind, change: Partial<ApiSourceInfo>) => {
  sources = sources.map((s) => (s.kind === kind ? { ...s, ...change } : s));
};
if (core) {
  core.handlers.set("api.list", () => structuredClone(sources));
  core.handlers.set("api.status", () => structuredClone(rows));
  core.handlers.set("api.connect", (args) => {
    patch((args as ApiConnectArgs).kind, { state: "connecting" });
    return { opId: `op-${++seq}`, labelKey: "op-api-connect", cancellable: true };
  });
  core.handlers.set("api.disconnect", (args) => {
    patch((args as { kind: ApiKind }).kind, { state: "disconnected", connected: false, account: null, error: null });
    return null;
  });
  core.handlers.set("api.getSettings", (args) => ({ kind: (args as { kind: ApiKind }).kind, values: structuredClone(settings[(args as { kind: ApiKind }).kind]) }));
  core.handlers.set("api.setSettings", (args) => {
    const a = args as ApiSetSettingsArgs;
    settings[a.kind] = a.settings.values;
    return { invalidated: false, confirmationRequired: false, dataStale: false };
  });
  core.handlers.set("crawl.parseList", (args) => {
    const lines = (args as { text: string }).text.split("\n").filter((l) => l.trim());
    return { valid: new Set(lines).size, invalid: 0, duplicates: lines.length - new Set(lines).size, sample: [] };
  });
  core.handlers.set("crawl.start", () => ({ crawlId: "c-apis", file: "/fixture/apis.ospider" }));
  registerPropertyCatalog({ ga4: async () => fixture.ga4Accounts as Ga4Account[] });
}

/** Beendet die letzte Verbindungsoperation wie ein Tick des Kerns. */
function finish(kind: ApiKind, outcome: { account: string } | { error: CmdError }) {
  if ("account" in outcome) patch(kind, { state: "connected", connected: true, account: outcome.account, accounts: [{ kind, account: outcome.account, accountId: `${kind}-1`, scopes: [] }], error: null });
  else patch(kind, { state: "error", connected: false, error: outcome.error });
  const opId = `op-${seq}`;
  useSession.setState((s) => ({ ops: { ...s.ops, [opId]: { opId, labelKey: "op-api-connect", done: 1, total: 1, state: "account" in outcome ? "done" : "failed", error: "error" in outcome ? outcome.error : null } } }));
}

const api = {
  calls: () => core?.calls ?? [],
  finish,
  setRows(next: ApiRow[]) {
    rows = next;
    useSession.setState((s) => ({ sidebarVersion: s.sidebarVersion + 1 }));
  },
  open: openApiAccess,
  action: (id: string) => runAction(id, { source: "menu" }),
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
  crawl: () => useSession.getState().crawl,
};
declare global {
  interface Window {
    integFixture: typeof api;
  }
}
window.integFixture = api;

createRoot(document.getElementById("root")!).render(
  <main>
    <header>
      <IntegrationsHeaderSlot />
    </header>
    <ApiSourceGear kind="ahrefs" />
    <IntegrationsHost />
  </main>,
);
