import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { registerAction, runAction, hasAction } from "../../ui/actions";
import { allMenuContributions } from "../../ui/menu/registry";
import { buildMenuModel, flattenItems } from "../../ui/menu/build";
import { makeT, setLanguage } from "../../i18n";
import { SHORTCUTS, shortcutTable } from "../../ui/shortcuts";
import { usePrefs } from "../../state/prefs";
import { useSession } from "../../state/session";
import { IpcError, localError } from "../../ipc/errors";
import { debugFixture, catalogFixture } from "./__fixtures__/fx-ui-basic";
import { HelpDialogs, ShortcutHelp, DebugHelp } from "./HelpDialogs";
import { installMainMenus } from "./install";
import { loadRecent, normalizeRecent } from "./recent";
import { useHelp } from "./state";
let core: MockCore;
let dispose: (() => void) | undefined;
beforeEach(() => {
  core = new MockCore();
  core.handlers.set("app.info", () => debugFixture.app);
  core.handlers.set("export.catalog", () => catalogFixture);
  core.handlers.set("mcp.status", () => ({ kind: "off" }));
  core.handlers.set("diag.debugInfo", () => debugFixture);
  core.handlers.set("diag.previewBundle", () => ({ files: ["logs/openspider.log", "system.txt", "config-diff.json", "tasks.json"], warnings: ["Redacted"] }));
  core.handlers.set("diag.bundle", () => ({ opId: "bundle-fixture" }));
  core.handlers.set("crawl.open", () => ({}));
  core.handlers.set("diag.usageStats", () => ({ crawls: 0, urls: 0, bytes: 0, perMonth: [] }));
  setClient(createClient(core.transport()));
  useHelp.setState({ page: null, path: null, message: null, operation: null });
  useSession.setState({ crawl: null, mcp: null, ops: {} });
});
afterEach(async () => { cleanup(); dispose?.(); dispose = undefined; setClient(null); vi.restoreAllMocks(); await setLanguage("en"); });

// spec: UI-113 [P1]
it("Hilfe-Dialog enthält alle Register-Einträge und Belegungen auf beiden Plattformen", async () => {
  for (const platform of ["mac", "win"] as const) for (const lang of ["de", "en"]) {
    await setLanguage(lang);
    const { container, unmount } = render(<ShortcutHelp platform={platform} />);
    const rows = container.querySelectorAll("[data-shortcut]");
    expect([...rows].map((row) => row.getAttribute("data-shortcut"))).toEqual(SHORTCUTS.map((s) => s.id));
    for (const [i, row] of shortcutTable(platform, lang).entries()) expect([...rows[i]!.querySelectorAll("kbd")].map((kbd) => kbd.textContent)).toEqual(row.keys);
    unmount();
  }
});

// spec: OUT-227 [M2]
it("Debug zeigt alle effektiven Pfade aus OPENSPIDER_HOME, keine konstruierten OS-Pfade", async () => {
  render(<DebugHelp />);
  for (const path of Object.values(debugFixture.app.paths)) if (path) expect(await screen.findByText(path)).toBeVisible();
  expect(screen.getByText("macOS fixture")).toBeVisible();
  expect(screen.getByText("Chromium")).toBeVisible();
  expect(screen.getByText("CPU cores")).toBeVisible();
});

// spec: OUT-241 [P1]
it("Log-Level wirkt beim Auswählen sofort; Aufbewahrung wird separat gespeichert", async () => {
  render(<DebugHelp />);
  const level = await screen.findByRole("combobox", { name: "Log level" });
  await waitFor(() => expect(level).toBeEnabled());
  fireEvent.change(level, { target: { value: "trace" } });
  await waitFor(() => expect(core.settings.logging.level).toBe("trace"));
  expect(core.settings.logging.retentionDays).toBe(30);
  await waitFor(() => expect(level).toBeEnabled());
  fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "14" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  await waitFor(() => expect(core.settings.logging.retentionDays).toBe(14));
  expect(core.settings.logging.level).toBe("trace");
});

// spec: OUT-244 [P1]
it("Diagnoseexport erst nach sichtbarer Dateivorschau und Bestätigung mit ZIP-Ziel", async () => {
  render(<DebugHelp />);
  await screen.findByText("macOS fixture");
  fireEvent.click(screen.getByRole("button", { name: "Save logs…" }));
  expect(await screen.findByText("system.txt")).toBeVisible();
  expect(screen.getByText("logs/openspider.log")).toBeVisible();
  expect(core.calls.filter((c) => c.cmd === "diag.bundle")).toHaveLength(0);
  fireEvent.change(screen.getByRole("textbox", { name: "Destination ZIP file" }), { target: { value: "/fixture/support.zip" } });
  fireEvent.click(screen.getByRole("button", { name: "Save these files" }));
  await waitFor(() => expect(core.calls.find((c) => c.cmd === "diag.bundle")?.args).toEqual({ dest: "/fixture/support.zip" }));
  expect(useHelp.getState().operation?.opId).toBe("bundle-fixture");
});

// spec: OUT-244 [P1]
it("Fehlgeschlagene Vorschau bietet keinen Diagnoseexport an", async () => {
  core.handlers.set("diag.previewBundle", () => { throw new IpcError(localError("Io", "err-code-io")); });
  render(<DebugHelp />);
  await screen.findByText("macOS fixture");
  fireEvent.click(screen.getByRole("button", { name: "Save logs…" }));
  await screen.findByRole("alert");
  expect(screen.queryByRole("button", { name: "Save these files" })).toBeNull();
  expect(core.calls.filter((c) => c.cmd === "diag.bundle")).toHaveLength(0);
});

// spec: CRAWL-127 [M2]
it("Öffnen aktualisiert zehn MRU-Einträge; fehlende Datei wird mit Hinweis entfernt", async () => {
  core.uiState = { version: 1, state: { foreign: { keep: true }, recentCrawls: Array.from({ length: 12 }, (_, i) => ({ path: `/fixture/${i}.ospider` })) } };
  dispose = installMainMenus();
  await waitFor(() => expect(hasAction("recent.9")).toBe(true));
  runAction("recent.3", { source: "menu" });
  await waitFor(async () => expect((await loadRecent())[0]?.path).toBe("/fixture/3.ospider"));
  expect((await loadRecent())).toHaveLength(10);
  expect((core.uiState.state as { foreign: unknown }).foreign).toEqual({ keep: true });
  core.handlers.set("crawl.open", () => { throw new IpcError(localError("NotFound", "err-code-notfound")); });
  runAction("recent.0", { source: "menu" });
  await waitFor(() => expect(useHelp.getState().message).toContain("/fixture/3.ospider"));
  expect((await loadRecent()).map((entry) => entry.path)).not.toContain("/fixture/3.ospider");
  expect(normalizeRecent([{ path: "x" }, { path: "x" }, {}, null])).toEqual([{ path: "x" }]);
});

// spec: CRAWL-127 [M2]
it("Speichern unter und Crawl-Export erzeugen Kopien über crawl.saveAs", async () => {
  core.handlers.set("crawl.saveAs", () => ({ opId: "copy" }));
  dispose = installMainMenus();
  for (const id of ["crawl.saveAs", "crawl.export"]) {
    runAction(id, { source: "menu" });
    await useHelp.getState().path?.submit("/fixture/copy.ospider");
    expect(core.calls.at(-1)).toMatchObject({ cmd: "crawl.saveAs", args: { dest: "/fixture/copy.ospider" } });
  }
});

// spec: UI-002
it("Hilfe-Aktionen öffnen echte Dialoge; leere Nutzungsstatistik zeigt keinen Fehler", async () => {
  dispose = installMainMenus();
  render(<HelpDialogs />);
  act(() => { runAction("help.usage", { source: "menu" }); });
  expect(await screen.findByText("No usage recorded yet")).toBeVisible();
  act(() => { runAction("app.about", { source: "menu" }); });
  expect(await screen.findByText("Apache 2.0 license")).toBeVisible();
  expect(screen.getByText("Third-party notices")).toBeVisible();
  expect(await screen.findByText("OpenSpider 0.1.0 fixture")).toBeVisible();
});

// spec: UI-002
it("Katalogeinträge rufen exakt den gelieferten Exportnamen auf; spätere Feature-Handler haben Vorrang", async () => {
  core.handlers.set("export.bulk", () => ({ opId: "bulk" }));
  dispose = installMainMenus();
  const id = "catalog.bulkExport.Pages With Uncrawlable Internal Outlinks";
  await waitFor(() => expect(hasAction(id)).toBe(true));
  runAction(id, { source: "menu" });
  await useHelp.getState().path?.submit("/fixture/out");
  expect(core.calls.at(-1)).toMatchObject({ cmd: "export.bulk", args: { name: "Pages With Uncrawlable Internal Outlinks", dir: "/fixture/out", format: "csv", singleWorkbook: false } });
  const handler = vi.fn(); const off = registerAction("crawl.open", handler);
  runAction("crawl.open"); expect(handler).toHaveBeenCalledTimes(1); off();
  dispose(); dispose = undefined;
  expect(hasAction(id)).toBe(false);
});

// spec: UI-002
it("MCP-Zustand wird bei Session- und Einstellungsänderungen neu markiert", async () => {
  await usePrefs.getState().load();
  dispose = installMainMenus();
  await waitFor(() => expect(useSession.getState().mcp?.kind).toBe("off"));
  useSession.setState({ mcp: { kind: "ready", port: 9000 } });
  const model = buildMenuModel(allMenuContributions(), { platform: "win", lang: "en", t: makeT("en"), ctx: {}, native: false });
  const all = flattenItems(model.menus.flatMap((m) => m.items));
  expect(all.find((i) => i.id === "mcp.start")?.enabled).toBe(false);
  expect(all.find((i) => i.id === "mcp.stop")?.enabled).toBe(true);
});

// spec: UI-002
it("Später registrierte Fachaktionen aktivieren ihren Menüeintrag und Abmeldung deaktiviert ihn", async () => {
  dispose = installMainMenus();
  const entry = () => flattenItems(buildMenuModel(allMenuContributions(), { platform: "win", lang: "en", t: makeT("en"), ctx: {}, native: false }).menus.flatMap((m) => m.items)).find((i) => i.id === "config.spider");
  expect(entry()?.enabled).toBe(false);
  expect(entry()?.disabledReason).toBeTruthy();
  const off = registerAction("config.spider", () => {});
  expect(entry()?.enabled).toBe(true);
  off();
  expect(entry()?.enabled).toBe(false);
});

// spec: UI-002
it("Alle fünf UI-Modi werden markiert, ohne unbekannte Modi an CrawlConfig zu senden", async () => {
  dispose = installMainMenus();
  for (const mode of ["spider", "list", "serp", "apis", "compare"]) {
    runAction(`mode.${mode}`, { source: "menu" });
    const menu = buildMenuModel(allMenuContributions(), { platform: "win", lang: "en", t: makeT("en"), ctx: {}, native: false }).menus.find((m) => m.id === "mode");
    expect(menu?.items.filter((i) => i.checked).map((i) => i.action)).toEqual([`mode.${mode}`]);
  }
  expect(core.calls.filter((c) => c.cmd === "config.set")).toHaveLength(0);
});

// spec: CRAWL-127 [M2]
it("Vorübergehender Lesefehler entfernt keinen Verlaufseintrag", async () => {
  core.uiState = { version: 1, state: { recentCrawls: [{ path: "/fixture/keep.ospider" }] } };
  core.handlers.set("crawl.open", () => { throw new IpcError(localError("Permission", "err-code-permission")); });
  dispose = installMainMenus();
  await waitFor(() => expect(hasAction("recent.0")).toBe(true));
  runAction("recent.0");
  await waitFor(() => expect(useHelp.getState().message).toBeTruthy());
  expect(await loadRecent()).toEqual([{ path: "/fixture/keep.ospider" }]);
});

// spec: CRAWL-127 [M2]
it("Öffnen während des Starts wartet auf den Verlauf und erhält ältere Einträge", async () => {
  let finishLoad!: (value: unknown) => void;
  core.handlers.set("uistate.load", () => new Promise((resolve) => { finishLoad = resolve; }));
  dispose = installMainMenus();
  runAction("crawl.open");
  const opening = useHelp.getState().path!.submit("/fixture/new.ospider");
  expect(core.calls.some((c) => c.cmd === "crawl.open")).toBe(false);
  core.uiState = { version: 1, state: { recentCrawls: [{ path: "/fixture/older.ospider" }] } };
  core.handlers.delete("uistate.load");
  finishLoad(core.uiState);
  await opening;
  expect(await loadRecent()).toEqual([{ path: "/fixture/new.ospider" }, { path: "/fixture/older.ospider" }]);
});

// spec: UI-002
it("Verspätetes Laden der Konfiguration überschreibt keine inzwischen gewählte Modusmarkierung", async () => {
  let finishConfig!: (value: unknown) => void;
  core.handlers.set("config.get", () => new Promise((resolve) => { finishConfig = resolve; }));
  dispose = installMainMenus();
  runAction("mode.serp");
  finishConfig({ schemaVersion: 2, generator: "fixture", config: { mode: "spider" } });
  await Promise.resolve(); await Promise.resolve();
  const menu = buildMenuModel(allMenuContributions(), { platform: "win", lang: "en", t: makeT("en"), ctx: {}, native: false }).menus.find((m) => m.id === "mode");
  expect(menu?.items.find((item) => item.id === "mode.serp")?.checked).toBe(true);
});

// spec: OUT-241 [P1]
it("Log-Level bleibt auch bei fehlgeschlagenen Systeminformationen bedienbar", async () => {
  core.handlers.set("diag.debugInfo", () => { throw new IpcError(localError("Io", "err-code-io")); });
  render(<DebugHelp />);
  const level = screen.getByRole("combobox", { name: "Log level" });
  await waitFor(() => expect(level).toBeEnabled());
  expect(screen.getByRole("alert")).toBeVisible();
  fireEvent.change(level, { target: { value: "warn" } });
  await waitFor(() => expect(core.settings.logging.level).toBe("warn"));
});
