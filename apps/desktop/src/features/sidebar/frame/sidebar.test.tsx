import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { AnalysisStatus } from "../../../ipc/gen/AnalysisStatus";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar/sidebar-dto.json";
import { registerAction } from "../../../ui/actions";
import { SidebarMount } from "./SidebarMount";
import { SidebarFrame } from "./SidebarFrame";
import { useSidebarNavigation } from "./state";
import { hasCount } from "../analysis/FilterStatus";
import { sortIssues } from "../issues/Issues";
import type { IssueRow } from "../../../ipc/gen/IssueRow";
import { documentationUrl } from "../issues/IssueExplanation";
import { tsv } from "./PanelActions";

let core: MockCore;
beforeEach(async () => {
  core = new MockCore();
  core.handlers.set("sidebar.overview", () => fixture.overview);
  core.handlers.set("sidebar.issues", () => fixture.issues);
  core.handlers.set("sidebar.issueExplain", () => fixture.explanation);
  core.handlers.set("analysis.status", () => fixture.analysis);
  core.handlers.set("analysis.start", () => ({ opId: "analyze-1", labelKey: "analysis", cancellable: true }));
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); vi.useRealTimers(); });
function mount() { return render(<Providers><SidebarFrame /></Providers>); }
async function tab(name: string) { fireEvent.click(await screen.findByRole("tab", { name })); }

// spec: UI-061
it("shows fixture summary in order, uses encountered as denominator and checks the robots/display identities", async () => {
  mount();
  const summary = await screen.findByRole("table", { name: "Summary" });
  const rows = within(summary).getAllByRole("row").slice(1);
  expect(rows.map(r => r.querySelector("th")?.textContent)).toEqual(fixture.overview.summary.map(s => s.key));
  expect(rows[4]).toHaveTextContent("27");
  expect(rows[4]).toHaveTextContent("93.1 %");
  expect(fixture.overview.summary[4]!.count).toBe(fixture.overview.summary[5]!.count + fixture.overview.summary[6]!.count);
  expect(fixture.overview.summary[0]!.count - fixture.overview.summary[4]!.count).toBe(2);
  expect(fixture.overview.summary[7]!.count + fixture.overview.summary[8]!.count).toBe(24); // internal HTML responses; one internal resource
  const titles = screen.getByRole("table", { name: "Page Titles" });
  expect(within(titles).getByRole("row", { name: /Duplicate/ })).toHaveTextContent("8.0 %");
  expect(within(titles).getByRole("row", { name: /Outside <head>/ })).toHaveTextContent("–");
  expect(screen.getByText("Incomplete – 7 pages unchecked")).toBeVisible();
  expect(screen.getByText("Low Content Pages").closest("tr")).toHaveClass("os-sidebar-stale");
});

// spec: UI-060
it("navigates with scope, scrolls up and requests table focus on double click; segment bars change the shared scope", async () => {
  const scrollTo = vi.fn();
  const main = document.createElement("main"); main.dataset.region = "table"; main.tabIndex = 0; main.scrollTo = scrollTo; document.body.append(main);
  useSidebarNavigation.getState().setScope({ segment: { kind: "segment", name: "Blog" } });
  mount();
  fireEvent.click(await screen.findByRole("button", { name: "Duplicate" }));
  expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Page Titles", filter: "Duplicate", segment: { kind: "segment", name: "Blog" } });
  expect(scrollTo).toHaveBeenCalledWith({ top: 0 });
  await tab("Issues");
  const row = await screen.findByRole("button", { name: "Response Codes: Internal Client Error (4xx)" });
  fireEvent.click(row); fireEvent.doubleClick(row);
  expect(useSidebarNavigation.getState().target?.focus).toBe(true);
  expect(main).toHaveFocus();
  fireEvent.click(screen.getAllByRole("button", { name: "Shop: 1" })[0]!);
  expect(useSidebarNavigation.getState().scope.segment).toEqual({ kind: "segment", name: "Shop" });
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.issues").at(-1)?.args).toMatchObject({ segment: { kind: "segment", name: "Shop" } }));
  main.remove();
});

// spec: UI-062
it("sorts fixture issues by type, priority, count and name and filters by all three controls", async () => {
  mount(); await tab("Issues");
  const table = await screen.findByRole("table");
  expect(within(table).getAllByRole("row").slice(1).map(r => r.querySelector("th button")?.textContent)).toEqual([
    "Response Codes: Internal Client Error (4xx)", "Images: Missing Alt Attribute", "Response Codes: Internal Redirection (3xx)", "Page Titles: Duplicate", "Meta Description: Missing",
  ]);
  expect(sortIssues([...fixture.issues, { ...fixture.issues[0]!, count: 0 }] as IssueRow[])).toHaveLength(5);
  expect(screen.getByText("More issues will be available after crawl analysis.")).toBeVisible();
  fireEvent.change(screen.getByLabelText("Issue Type"), { target: { value: "opportunity" } });
  fireEvent.change(screen.getByLabelText("Issue Priority"), { target: { value: "medium" } });
  fireEvent.change(screen.getByLabelText("Search issues"), { target: { value: "DUPLICATE" } });
  await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
  expect(screen.getByRole("button", { name: "Page Titles: Duplicate" })).toBeVisible();
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.issues").at(-1)?.args).toMatchObject({ type: "opportunity", priority: "medium", search: "DUPLICATE" }));
});

// spec: UI-065 [M2]
it("renders only core explanation text, resolved threshold, rationale and a safe documentation link, reloading on language/version changes", async () => {
  mount(); await tab("Issues");
  fireEvent.click(await screen.findByRole("button", { name: "Page Titles: Duplicate" }));
  const explanation = await screen.findByRole("region", { name: "Issue explanation" });
  expect(await within(explanation).findByText(fixture.explanation.what)).toBeVisible();
  expect(explanation).toHaveTextContent("65");
  expect(explanation.textContent).not.toMatch(/\{[a-z_]+\}/);
  expect(explanation).toHaveTextContent(fixture.explanation.why);
  expect(explanation).toHaveTextContent(fixture.explanation.fix);
  expect(explanation).toHaveTextContent(fixture.explanation.rule);
  expect(within(explanation).getByRole("link")).toHaveAttribute("href", fixture.explanation.learnMoreUrl);
  await act(async () => { await setLanguage("de"); });
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.issueExplain").at(-1)?.args).toMatchObject({ lang: "de" }));
  expect(documentationUrl("javascript:alert(1)")).toBeNull();
});

// spec: UI-082 [M2]
it("keeps old values grey, shows stale steps from the core and starts only a user requested reanalysis", async () => {
  mount();
  expect(await screen.findByText("Analysis outdated – analyze again")).toBeVisible();
  expect(core.calls.some(c => c.cmd === "analysis.start")).toBe(false);
  expect(screen.queryByRole("tab", { name: "Analysis" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Low Content Pages" }).closest("tr")).toHaveTextContent("1");
  fireEvent.click(screen.getByRole("button", { name: "Analyze again" }));
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "analysis.start").at(-1)?.args).toEqual({ steps: null }));
  const before = core.calls.filter(c => c.cmd === "analysis.start").length;
  act(() => useSidebarNavigation.getState().setScope({ segment: { kind: "segment", name: "Shop" } }));
  expect(core.calls.filter(c => c.cmd === "analysis.start")).toHaveLength(before);
});

// spec: ONPAGE-140 [M2]
it("uses the core needsRecrawl state for the saved HTML notice and points at the storage setting", async () => {
  const open = vi.spyOn(window, "open").mockReturnValue(null);
  mount();
  expect(await screen.findByText("Reanalysis needs saved HTML. Recrawl required.")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "HTML storage setting" }));
  expect(open).toHaveBeenCalledWith(expect.stringContaining("#extraction.store_html"), "settings", expect.any(String));
});

// spec: UI-080
it("updates the sidebar notice from core ticks and never replaces pending filter counts with zero", async () => {
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  mount();
  await screen.findByRole("table", { name: "Summary" });
  expect(screen.queryByText("Analysis outdated – analyze again")).not.toBeInTheDocument();
  act(() => useSession.setState({ analysis: { ...fixture.analysis, running: true, current: "link_score", percent: 42 } as AnalysisStatus }));
  expect(await screen.findByRole("button", { name: "Analyze again" })).toBeDisabled();
  await tab("Issues");
  expect(screen.getByRole("button", { name: "Analyze again" })).toBeDisabled();
  expect(screen.getByText("Reanalysis needs saved HTML. Recrawl required.")).toBeVisible();
  expect(hasCount({ kind: "needsCrawlAnalysis" })).toBe(false);
  expect(hasCount({ kind: "needsRecrawl" })).toBe(false);
  expect(hasCount({ kind: "incomplete", unchecked: 7 })).toBe(true);
});

// spec: UI-060, UI-080
it("mounts only the specified sidebar tabs and leaves statusbar composition to U03", async () => {
  core.uiState = { version: 1, state: { sidebar: { selected: "analysis", order: ["analysis", "issues"] } } };
  useSession.setState({ analysis: { ...fixture.analysis, running: true, percent: 42 } as AnalysisStatus });
  render(<Providers><div><aside className="os-app-sidebar" /><footer className="os-app-status">U03 status</footer><SidebarMount /></div></Providers>);
  await screen.findByRole("button", { name: "Page Titles: Duplicate" });
  expect(screen.getAllByRole("tab").map(t => t.textContent)).toEqual(["Issues", "Overview", "Site Structure", "Segments", "Response Times", "API", "Spelling & Grammar"]);
  expect(screen.getByRole("contentinfo").textContent).toBe("U03 status");
  expect(screen.getByRole("contentinfo").children).toHaveLength(0);
});

// spec: UI-060
it("copies displayed rows and dispatches export to the shared UI action", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  mount(); await tab("Issues");
  fireEvent.click(await screen.findByRole("button", { name: "Copy" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith(expect.stringContaining("Issue Name\tIssue Type\tIssue Priority\tURLs\t% of Total")));
  expect(tsv([["A\nB", "C\tD"]])).toBe("A B\tC D");
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  expect(await screen.findByText("Export is not available in this build yet.")).toBeVisible();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(core.calls.some(c => c.cmd.startsWith("export."))).toBe(false);
  const handler = vi.fn();
  const unregister = registerAction("export.sidebar", handler);
  try {
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect(handler).toHaveBeenCalledWith({ source: "ui" });
    expect(screen.queryByText("Export is not available in this build yet.")).not.toBeInTheDocument();
  } finally { unregister(); }
});

// spec: UI-061
it("preserves collapsed blocks across tabs and saves its namespace without deleting other UI preferences", async () => {
  vi.useFakeTimers();
  core.uiState = { version: 1, state: { table: { filter: "All" } } };
  mount();
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  fireEvent.click(screen.getByRole("button", { name: /▾ Page Titles/ }));
  expect(screen.queryByRole("table", { name: "Page Titles" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("tab", { name: "Issues" }));
  fireEvent.click(screen.getByRole("tab", { name: "Overview" }));
  expect(screen.queryByRole("table", { name: "Page Titles" })).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(1001); });
  expect(core.uiState.state).toMatchObject({ table: { filter: "All" }, sidebar: { collapsed: ["Page Titles"] } });
});

// spec: UI-060
it("reloads counts on ticks and ignores a late reply from an earlier scope", async () => {
  let finish: (data: OverviewDto) => void = () => undefined;
  core.handlers.set("sidebar.overview", args => (args as { segment?: unknown }).segment ? { ...fixture.overview, summary: [{ key: "Total URLs Encountered", count: 3 }] } : new Promise<OverviewDto>(resolve => { finish = resolve; }));
  mount();
  await waitFor(() => expect(core.calls.some(c => c.cmd === "sidebar.overview")).toBe(true));
  act(() => useSidebarNavigation.getState().setScope({ segment: { kind: "segment", name: "Shop" } }));
  expect(await screen.findByRole("table", { name: "Summary" })).toHaveTextContent("3");
  await act(async () => finish(fixture.overview as OverviewDto));
  expect(screen.getByRole("table", { name: "Summary" })).not.toHaveTextContent("29");
  const count = core.calls.filter(c => c.cmd === "sidebar.overview").length;
  act(() => useSession.getState().applyTick({ ...tick, seq: 99, countsVersion: 20 } as Parameters<ReturnType<typeof useSession.getState>["applyTick"]>[0]));
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.overview").length).toBeGreaterThan(count));
});

// spec: UI-060
it("supports hiding, restoring and keyboard reordering tabs, and gates the semantic panel on embeddings", async () => {
  render(<Providers><SidebarFrame panels={[{ id: "semanticSearch", titleKey: "sidebar-tab-semanticSearch", Component: () => <p>Semantic panel</p>, embeddingsOnly: true }]} /></Providers>);
  await waitFor(() => expect(document.querySelector('[data-sidebar-ready="true"]')).toBeInTheDocument());
  expect(screen.queryByRole("tab", { name: "Semantic Search" })).not.toBeInTheDocument();
  fireEvent.contextMenu(screen.getByRole("tab", { name: "Overview" }).querySelector("span")!);
  fireEvent.click(screen.getByRole("button", { name: "Hide Overview" }));
  expect(screen.queryByRole("tab", { name: "Overview" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Customize tabs"));
  fireEvent.click(screen.getByRole("checkbox", { name: /Overview/ }));
  expect(screen.getByRole("tab", { name: "Overview" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Move to first position: Issues" }));
  expect(screen.getAllByRole("tab")[0]).toHaveTextContent("Issues");
});

// spec: UI-060
it("shows core errors with retry instead of presenting fabricated counts", async () => {
  core.handlers.set("sidebar.overview", () => { throw new Error("fixture failure"); });
  mount();
  expect(await screen.findByRole("alert")).toBeVisible();
  expect(screen.queryByRole("table", { name: "Summary" })).not.toBeInTheDocument();
  core.handlers.set("sidebar.overview", () => fixture.overview);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(await screen.findByRole("table", { name: "Summary" })).toHaveTextContent("29");
});

// spec: UI-080
it("keeps an accepted analysis start disabled until the core finishes the operation", async () => {
  mount();
  const start = await screen.findByRole("button", { name: "Analyze again" });
  fireEvent.click(start);
  await waitFor(() => expect(core.calls.some(c => c.cmd === "analysis.start")).toBe(true));
  expect(start).toBeDisabled();
  act(() => useSession.setState({ ops: { "analyze-1": { opId: "analyze-1", labelKey: "analysis", done: 1, state: "done" } } }));
  expect(start).toBeEnabled();
});

// spec: UI-082 [M2]
it("marks only filters belonging to invalidated steps stale before a counts tick arrives", async () => {
  const overview = structuredClone(fixture.overview) as OverviewDto;
  overview.tabs[1]!.filters[1]!.analysisStep = "near_duplicates";
  core.handlers.set("sidebar.overview", () => overview);
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  mount();
  const duplicate = await screen.findByRole("button", { name: "Duplicate" });
  expect(duplicate.closest("tr")).not.toHaveClass("os-sidebar-stale");
  act(() => useSession.setState({ analysis: { running: false, steps: [{ step: "near_duplicates", state: "done", stale: true }, { step: "link_score", state: "done", stale: false }] } }));
  expect(duplicate.closest("tr")).toHaveClass("os-sidebar-stale");
  expect(duplicate.closest("tr")).toHaveTextContent("2");
  expect(screen.getByRole("button", { name: "Indexable" }).closest("tr")).not.toHaveClass("os-sidebar-stale");
});

// spec: UI-060, UI-061, UI-062
it("localizes the percent heading in summary, filter and issue tables and copied rows", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  mount();
  await screen.findByRole("table", { name: "Summary" });
  await act(async () => { await setLanguage("de"); });
  const tables = screen.getAllByRole("table");
  for (const table of tables) expect(within(table).getByRole("columnheader", { name: "% gesamt" })).toBeVisible();
  expect(screen.queryByRole("columnheader", { name: "% of Total" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Kopieren" }));
  await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(expect.stringContaining("Name\tURLs\t% gesamt")));
  await tab("Befunde");
  expect(await screen.findByRole("columnheader", { name: "% gesamt" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Kopieren" }));
  await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(expect.stringContaining("Befundname\tBefundtyp\tPriorität\tURLs\t% gesamt")));
});
