// Test-only page served by Vite; destination is supplied by the test, never by a production URL parameter.
import { createRoot } from "react-dom/client";
import { currentPlatform } from "../../src/ui/platform";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import type { ExportPreset } from "../../src/ipc/gen/ExportPreset";
import type { ExportRequest } from "../../src/features/export/model";
import { openExport, registerExportContext } from "../../src/features/export";
import { ExportButton, ExportHost, ExportViewButton } from "../../src/features/export";
import type { ExportDesktop, DestinationOptions } from "../../src/features/export/native";
import { useSession } from "../../src/state/session";
import fixture from "../../src/features/export/fixtures/out-basic-site.json";
import "../../src/ui/ui.css";

const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage("en");
const core = window.__osMock;
const handle = { opId: "export-fixture", labelKey: "export-progress", cancellable: true };
if (core) {
  for (const command of ["view", "detail", "bulk", "report", "sidebar", "issues", "sitemap", "multi"]) core.handlers.set(`export.${command}`, () => handle);
  core.handlers.set("export.suggestName", args => (args as { target: { format?: string } }).target.format === "xlsx" ? fixture.name.replace(".csv", ".xlsx") : fixture.name);
  core.handlers.set("export.catalog", () => fixture.catalog);
  core.handlers.set("catalog.tabs", () => fixture.tabs);
  let saved = JSON.parse(localStorage.getItem("export-fixture-presets") ?? "[]") as ExportPreset[];
  core.handlers.set("export.presetList", () => saved);
  core.handlers.set("export.presetSave", args => {
    const { preset } = args as { preset: ExportPreset };
    saved = [...saved.filter(p => p.name !== preset.name), preset];
    localStorage.setItem("export-fixture-presets", JSON.stringify(saved));
  });
  core.handlers.set("export.presetDelete", args => { saved = saved.filter(p => p.name !== (args as { name: string }).name); localStorage.setItem("export-fixture-presets", JSON.stringify(saved)); });
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: { sitemap_gen: {} } }));
  core.handlers.set("config.set", () => ({ invalidatedSteps: [], deferred: [] }));
}
let path: string | null = "/fixture/output.csv";
const destinations: DestinationOptions[] = [];
const reveals: string[] = [];
const desktop: ExportDesktop = {
  choose: async options => { destinations.push(options); return path; },
  directory: async () => "/fixture",
  reveal: async p => { reveals.push(p); },
};
registerExportContext("view", fixture.view as ExportRequest & { kind: "view" });
const api = {
  platform: currentPlatform(),
  setDestination(value: string | null) { path = value; },
  destinations, reveals,
  calls: () => core?.calls ?? [],
  open: openExport,
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
  phase: () => useSession.getState().crawl?.phase,
  complete() { useSession.setState({ ops: { [handle.opId]: { ...handle, state: "done", done: 3, total: 3, result: { path: path ?? "/fixture/output.csv", bytes: 100 } } } }); },
};
declare global { interface Window { exportFixture: typeof api } }
window.exportFixture = api;
createRoot(document.getElementById("root")!).render(<main>
  <ExportViewButton />
  <ExportButton request={fixture.detail as ExportRequest} />
  <button onClick={() => openExport({ kind: "multi" })}>Multi export</button>
  <button onClick={() => openExport({ kind: "sitemap", sitemapKind: "pages", listMode: true, hreflangAvailable: false })}>Sitemap</button>
  <ExportHost desktop={desktop} />
</main>);
