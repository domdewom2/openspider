import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { setLanguage } from "../../i18n";
import { usePrefs } from "../../state/prefs";
import { useSession } from "../../state/session";
import { runAction } from "../../ui/actions";
import type { ExportRequest } from "./model";
import { openExport, registerExportContext, useExportContext, useExportDialog } from "./model";
import { ExportDialog } from "./ExportDialog";
import { ExportButton, ExportHost, ExportViewButton, menuLabel } from "./ExportHost";
import { executeExport, type ExportChoices } from "./execute";
import { defaultSitemapOptions, sitemapSchema } from "./sitemap";
import { nativeExportDesktop, type ExportDesktop } from "./native";
import fixture from "./fixtures/out-basic-site.json";

let core: MockCore;
let desktop: ExportDesktop;
const handle = { opId: "export-1", labelKey: "export-progress", cancellable: true };
const view = fixture.view as ExportRequest;
const detail = fixture.detail as ExportRequest;
const defaults = (): ExportChoices => ({ format: "csv", allColumns: false, preserveUploadOrder: true, singleWorkbook: false, skipEmpty: false, items: [], sitemap: defaultSitemapOptions() });
const call = (name: string) => core.calls.filter(c => c.cmd === name);
function mount(request: ExportRequest) {
  const onStarted = vi.fn();
  const onClose = vi.fn();
  render(<ExportDialog request={request} onClose={onClose} onStarted={onStarted} desktop={desktop} />);
  return { onStarted, onClose };
}
async function save() {
  const button = await screen.findByRole("button", { name: "Save…" });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
}
beforeEach(async () => {
  core = new MockCore();
  setClient(createClient(core.transport("main")));
  await setLanguage("en");
  await usePrefs.getState().load();
  useSession.setState({ ops: {}, crawl: null, countsVersion: 0 });
  useExportDialog.setState({ request: null });
  useExportContext.setState({ view: null, detail: null, sidebar: null });
  for (const command of ["view", "detail", "bulk", "report", "sidebar", "issues", "sitemap", "multi"]) core.handlers.set(`export.${command}`, () => handle);
  core.handlers.set("export.suggestName", () => fixture.name);
  core.handlers.set("export.catalog", () => fixture.catalog);
  core.handlers.set("catalog.tabs", () => fixture.tabs);
  core.handlers.set("export.presetList", () => [fixture.preset]);
  core.handlers.set("export.presetSave", () => null);
  core.handlers.set("export.presetDelete", () => null);
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: { sitemap_gen: {}, spider: { hreflang: { store: false } }, mode: "list", seed: "keep" } }));
  core.handlers.set("config.set", () => ({ invalidatedSteps: [], deferred: [] }));
  desktop = { choose: vi.fn(async () => "/fixture/export.csv"), directory: vi.fn(async () => "/fixture"), reveal: vi.fn(async () => undefined) };
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });

describe("Export contracts", () => {
  // spec: OUT-020 [M2]
  it("exports the captured filtered/sorted/segmented snapshot and visible column order", async () => {
    await executeExport(view, "/fixture/view.csv", defaults());
    expect(call("export.view")[0]?.args).toEqual({ viewId: "titles-blog-sorted", version: 7, path: "/fixture/view.csv", format: "csv", allColumns: false, columns: ["Address", "Title 1"], preserveUploadOrder: true });
    await executeExport(view, "/fixture/all.xlsx", { ...defaults(), format: "xlsx", allColumns: true });
    expect(call("export.view")[1]?.args).toMatchObject({ allColumns: true, columns: null, format: "xlsx" });
  });
  // spec: OUT-021 [M2]
  it("passes all selected URLs and the P1 detail table spec without fetching rows", async () => {
    await executeExport(detail, "/fixture/cookies.csv", defaults());
    expect(call("export.detail")[0]?.args).toEqual({ spec: fixture.detail.spec, selection: fixture.detail.selection, path: "/fixture/cookies.csv", format: "csv" });
    const all: ExportRequest = { ...fixture.detail, kind: "detail", spec: { kind: "inlinks", urlIds: [], filter: { id: "All" }, sort: [] }, selection: { kind: "all", viewId: "v", version: 7, except: [11] } } as ExportRequest;
    await executeExport(all, "/fixture/inlinks.csv", defaults());
    expect(call("export.detail")[1]?.args).toMatchObject({ selection: all.kind === "detail" ? all.selection : null });
    expect(core.calls.some(c => c.cmd === "view.rows")).toBe(false);
  });
  // spec: OUT-006
  it("routes bulk, reports, sidebar and issues to their typed commands", async () => {
    for (const request of [{ kind: "bulk", name: "Links:All Inlinks" }, { kind: "report", name: "Crawl Overview" }, { kind: "sidebar", which: "overview" }, { kind: "issues", which: "issues" }] as ExportRequest[]) {
      await executeExport(request, "/fixture", defaults());
      expect(call(`export.${request.kind}`)).toHaveLength(1);
    }
  });
  // spec: OUT-007 [P1]
  it("allows consolidation only for XLSX and preserves multi selection order", async () => {
    const request: ExportRequest = { kind: "multi" };
    await executeExport(request, "/fixture", { ...defaults(), items: fixture.preset.items, singleWorkbook: true });
    expect(call("export.multi")[0]?.args).toMatchObject({ singleWorkbook: false, items: fixture.preset.items });
    await executeExport(request, "/fixture", { ...defaults(), format: "xlsx", items: fixture.preset.items, singleWorkbook: true, skipEmpty: true });
    expect(call("export.multi")[1]?.args).toMatchObject({ singleWorkbook: true, skipEmpty: true });
  });
});

// spec: OUT-006
it("uses the suggested name and persists accepted format/directory across reload", async () => {
  mount(view);
  await save();
  await waitFor(() => expect(call("export.view")).toHaveLength(1));
  expect(desktop.choose).toHaveBeenCalledWith(expect.objectContaining({ defaultName: fixture.name, directory: null, extension: "csv", folder: false }));
  await waitFor(() => expect(core.settings.export.directory).toBe("/fixture"));
  cleanup();
  const restarted = new MockCore(core.settings);
  restarted.handlers.set("export.suggestName", () => fixture.name);
  setClient(createClient(restarted.transport("main")));
  await usePrefs.getState().load();
  mount(view);
  await save();
  expect(desktop.choose).toHaveBeenLastCalledWith(expect.objectContaining({ directory: "/fixture", extension: "csv" }));
});

// spec: OUT-006
it("writes nothing when the native save/overwrite dialog is cancelled", async () => {
  desktop.choose = vi.fn(async () => null);
  const { onStarted } = mount(view);
  await save();
  await waitFor(() => expect(desktop.choose).toHaveBeenCalledTimes(1));
  expect(call("export.view")).toHaveLength(0);
  expect(call("settings.set")).toHaveLength(0);
  expect(onStarted).not.toHaveBeenCalled();
});

// spec: OUT-006
it("uses the installed native dialog plugin and delegates overwrite handling to the OS", async () => {
  const invoke = vi.fn(async () => null);
  setClient({ transportName: "tauri", invoke: invoke as never, subscribe: () => () => undefined });
  const result = await nativeExportDesktop.choose({ title: "Save", defaultName: "view.csv", directory: null, extension: "csv", folder: false });
  expect(result).toBeNull();
  expect(invoke).toHaveBeenCalledWith("dialog.save", { options: { title: "Save", defaultPath: "view.csv", filters: [{ name: "CSV", extensions: ["csv"] }] } }, undefined);
  await nativeExportDesktop.choose({ title: "Folder", defaultName: "multi", directory: null, extension: "xlsx", folder: true });
  expect(invoke).toHaveBeenLastCalledWith("dialog.open", expect.objectContaining({ options: expect.objectContaining({ directory: true, multiple: false }) }), undefined);
});

// spec: OUT-020 [M2]
it("offers all columns and forwards the unchanged backend date/segment filename", async () => {
  mount(view);
  fireEvent.click(await screen.findByRole("checkbox", { name: /all columns/ }));
  await save();
  await waitFor(() => expect(call("export.view")[0]?.args).toMatchObject({ allColumns: true, columns: null }));
  expect(call("export.suggestName")[0]?.args).toEqual({ target: { kind: "view", tab: "Page Titles", filter: "Missing", format: "csv" } });
});

// spec: UI-015
it("places an export trigger above the view and resolves Mod+E through the shared action", async () => {
  const dispose = registerExportContext("view", view as Extract<ExportRequest, { kind: "view" }>);
  render(<><ExportViewButton /><ExportHost desktop={desktop} /></>);
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  expect(await screen.findByRole("dialog", { name: "Export options" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  act(() => { expect(runAction("export.view", { source: "shortcut" })).toBe(true); });
  expect(await screen.findByRole("dialog")).toBeInTheDocument();
  act(dispose);
});

// spec: OUT-021 [P1]
it("exposes detail export and explains missing data without opening a dialog", () => {
  render(<ExportButton request={detail} disabledReason="export-hreflang-required" />);
  expect(screen.getByRole("button", { name: "Export" })).toBeDisabled();
  expect(screen.getByTitle("Enable hreflang extraction for the crawl.")).toBeInTheDocument();
});

// spec: OUT-007 [P1]
it("disables the workbook option for CSV", async () => {
  mount({ kind: "multi" });
  expect(await screen.findByRole("checkbox", { name: "Combine in one workbook" })).toBeDisabled();
});

// spec: OUT-075 [P1]
it("searches the export tree, loads and saves named presets and keeps unknown entries for the backend warning", async () => {
  const unknown = { ...fixture.preset, items: [...fixture.preset.items, { kind: "bulk", name: "New:Unknown" }] };
  core.handlers.set("export.presetList", () => [unknown]);
  mount({ kind: "multi" });
  expect(await screen.findByRole("group", { name: "Internal" })).toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "Search exports" }), { target: { value: "Inlinks" } });
  expect(screen.getByRole("checkbox", { name: "Links:All Inlinks" })).toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "All" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /Preset/ }));
  fireEvent.click(await screen.findByRole("option", { name: "Daily" }));
  expect(await screen.findByText(/New:Unknown/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save preset" }));
  await waitFor(() => expect(call("export.presetSave")[0]?.args).toMatchObject({ name: "Daily", preset: unknown }));
  await save();
  await waitFor(() => expect(call("export.multi")[0]?.args).toMatchObject({ items: unknown.items, skipEmpty: true, singleWorkbook: false }));
});

// spec: OUT-124 [P1]
it("rejects priority 1.5 and fractional steps, and explains list mode depth zero", async () => {
  mount({ kind: "sitemap", sitemapKind: "pages", listMode: true, hreflangAvailable: true });
  await screen.findByText(fixture.name, { exact: false });
  fireEvent.click(screen.getByRole("tab", { name: "Include priority" }));
  expect(screen.getByRole("note")).toHaveTextContent("all URLs have depth 0");
  const input = screen.getByRole("spinbutton", { name: "Depth 0" });
  fireEvent.change(input, { target: { value: "1.5" } });
  expect(screen.getByRole("button", { name: "Save…" })).toBeDisabled();
  expect(input).toHaveAttribute("aria-invalid", "true");
  fireEvent.change(input, { target: { value: "0.55" } });
  expect(screen.getByRole("button", { name: "Save…" })).toBeDisabled();
  fireEvent.change(input, { target: { value: "0.8" } });
  await waitFor(() => expect(screen.getByRole("button", { name: "Save…" })).toBeEnabled());
  await save();
  await waitFor(() => expect(call("export.sitemap")[0]?.args).toMatchObject({ sitemapKind: "pages", options: { priority: { enabled: true, by_depth: [0.8, 0.9, 0.8, 0.7, 0.6, 0.5] } } }));
  expect(call("config.set")[0]?.args).toMatchObject({ config: { seed: "keep", sitemap_gen: { priority: { enabled: true } } } });
});

// spec: OUT-128 [P1]
it("disables hreflang without extraction and retains the valid-target filter default", async () => {
  mount({ kind: "sitemap", sitemapKind: "pages", listMode: false, hreflangAvailable: false });
  fireEvent.click(screen.getByRole("tab", { name: "Include hreflang" }));
  expect(screen.getByRole("checkbox", { name: "Include hreflang" })).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: /Only indexable/ })).toBeChecked();
  expect(screen.getByRole("note")).toHaveTextContent("Enable hreflang extraction");
  await save();
  await waitFor(() => expect(call("export.sitemap")[0]?.args).toMatchObject({ options: { hreflang: { enabled: false, only_valid_targets: true } } }));
});

// spec: OUT-128 [P1]
it("enables collected hreflang and serializes image sitemap settings using core field names", async () => {
  mount({ kind: "sitemap", sitemapKind: "images", listMode: false, hreflangAvailable: true });
  await waitFor(() => expect(screen.getByRole("button", { name: "Save…" })).toBeEnabled());
  fireEvent.click(screen.getByRole("tab", { name: "Include hreflang" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Include hreflang" }));
  await save();
  await waitFor(() => expect(call("export.sitemap")[0]?.args).toMatchObject({ sitemapKind: "images", options: { images: { enabled: true }, hreflang: { enabled: true, only_valid_targets: true }, changefreq: { basis: "last_modified" } } }));
});

// spec: OUT-124 [P1]
it("validates sitemap defaults and calendar dates against a fixture", () => {
  const options = defaultSitemapOptions();
  expect(options.priority.by_depth).toEqual([1, 0.9, 0.8, 0.7, 0.6, 0.5]);
  options.lastmod = { enabled: true, source: "custom", custom_date: "2026-02-30" };
  expect(sitemapSchema.safeParse(options).success).toBe(false);
  options.lastmod.custom_date = "2026-10-13";
  expect(sitemapSchema.safeParse(options).success).toBe(true);
});

// spec: OUT-006
it("shows background progress, cancels only its operation, and reveals the completed result", async () => {
  core.handlers.set("op.cancel", () => null);
  render(<ExportHost desktop={desktop} />);
  act(() => openExport(view));
  await save();
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  act(() => useSession.setState({ ops: { [handle.opId]: { ...handle, state: "running", done: 4, total: 10 } } }));
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "4");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(call("op.cancel")[0]?.args).toEqual({ opId: handle.opId }));
  expect(screen.queryByRole("button", { name: "Open folder" })).not.toBeInTheDocument();
  act(() => useSession.setState({ ops: { [handle.opId]: { ...handle, state: "done", done: 10, total: 10, result: { path: "/actual/view.csv", bytes: 200 } } } }));
  fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
  await waitFor(() => expect(desktop.reveal).toHaveBeenCalledWith("/actual/view.csv"));
});

// spec: OUT-006
it("reports operation failure and cancellation without offering a success action", async () => {
  render(<ExportHost desktop={desktop} />);
  act(() => openExport(view));
  await save();
  await waitFor(() => expect(call("export.view")).toHaveLength(1));
  act(() => useSession.setState({ ops: { [handle.opId]: { ...handle, state: "failed", done: 0, error: { code: "Io", messageKey: "err-io", args: {}, retryable: true } } } }));
  expect(await screen.findByRole("alert")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Open folder" })).not.toBeInTheDocument();
  act(() => useSession.setState({ ops: { [handle.opId]: { ...handle, state: "cancelled", done: 0 } } }));
  expect(await screen.findByText("Export cancelled.")).toBeInTheDocument();
});

// spec: OUT-006
it("blocks duplicate starts while the native dialog is pending", async () => {
  let resolve!: (path: string | null) => void;
  desktop.choose = vi.fn(() => new Promise<string | null>(r => { resolve = r; }));
  mount(view);
  await save();
  fireEvent.click(screen.getByRole("button", { name: "Save…" }));
  expect(desktop.choose).toHaveBeenCalledTimes(1);
  await act(async () => resolve("/fixture/view.csv"));
  await waitFor(() => expect(call("export.view")).toHaveLength(1));
});

// spec: OUT-006
it("remembers XLSX after selection and recovers a failed name lookup", async () => {
  let attempts = 0;
  core.handlers.set("export.suggestName", args => {
    if (attempts++ === 0) throw new Error("transient name lookup");
    const { target } = args as { target: { format: string } };
    return target.format === "xlsx" ? fixture.name.replace(".csv", ".xlsx") : fixture.name;
  });
  mount(view);
  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Save…" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: /File format/ }));
  fireEvent.click(await screen.findByRole("option", { name: "Excel Workbook (.xlsx)" }));
  await save();
  await waitFor(() => expect(core.settings.export.format).toBe("xlsx"));
  expect(desktop.choose).toHaveBeenCalledWith(expect.objectContaining({ extension: "xlsx", defaultName: fixture.name.replace(".csv", ".xlsx") }));
});

// spec: UI-002
// spec: UI-121
it("labels catalog entries without a translation by their name and keeps translated keys (A01-22)", () => {
  expect(menuLabel("Accessibility:WCAG 2.0 A", "Accessibility:")).toEqual({ label: "WCAG 2.0 A" });
  expect(menuLabel("Links:All Inlinks")).toEqual({ label: "Links:All Inlinks" });
  expect(menuLabel("export-multi")).toEqual({ labelKey: "export-multi" });
});
