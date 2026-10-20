import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { ApiRow } from "../../../ipc/gen/ApiRow";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar-fach/fach-dto.json";
import { SidebarFrame } from "../frame/SidebarFrame";
import { useSidebarNavigation } from "../frame/state";
import { useIntegrations } from "../../integrations";
import { API_ORDER, apiStatus, panelRows, progressPercent, redact } from "./model";

let core: MockCore;
let rows: ApiRow[];
beforeEach(async () => {
  rows = structuredClone(fixture.api) as ApiRow[];
  core = new MockCore();
  core.handlers.set("sidebar.overview", () => ({ summary: [], tabs: [] }));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("sidebar.apiStatus", () => structuredClone(rows));
  core.handlers.set("api.errors", () => fixture.apiErrors);
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  useIntegrations.setState({ dialog: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });

async function open() {
  render(<Providers><SidebarFrame /></Providers>);
  fireEvent.click(await screen.findByRole("tab", { name: "API" }));
  return screen.findByRole("table", { name: "API data sources" });
}

// spec: UI-070
it("keeps the fixed row order with Majestic, Ahrefs and Moz as three rows even when the core reports nothing", () => {
  expect(API_ORDER).toEqual(["ga4", "gsc", "psi", "majestic", "ahrefs", "moz"]);
  const out = panelRows([{ ...(fixture.api[2] as ApiRow) }]);
  expect(out.map(r => r.kind)).toEqual([...API_ORDER]);
  expect(out.filter(r => ["majestic", "ahrefs", "moz"].includes(r.kind)).map(apiStatus)).toEqual(["disconnected", "disconnected", "disconnected"]);
});

// spec: UI-070
it("derives status and progress: running, done with final errors, quota exhausted, not connected", () => {
  const [ga4, gsc, psi, majestic] = fixture.api as ApiRow[];
  expect([ga4, gsc, psi, majestic].map(r => apiStatus(r!))).toEqual(["running", "quotaExhausted", "done", "disconnected"]);
  expect(apiStatus({ ...psi!, received: 0, completed: 4, requested: 10, errors: 4 })).toBe("error");
  expect(apiStatus({ ...psi!, errors: 0, received: 0, completed: 0, requested: 0 })).toBe("connected");
  // Final errors count as settled: 10 of 10 requested, 2 errors → 100 % (INTEG-007).
  expect([ga4, psi].map(r => progressPercent(r!))).toEqual([80, 100]);
  expect(progressPercent({ ...psi!, requested: 0, completed: 0 })).toBe(0);
});

// spec: UI-070
it("renders the specified columns, statuses, quota reset time and a gear per row", async () => {
  const table = await open();
  expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["API", "Status", "Requested", "Received", "Errors", "Progress"]);
  const body = within(table).getAllByRole("row").slice(1);
  expect(body).toHaveLength(6);
  expect(body.map(r => r.getAttribute("data-api-kind"))).toEqual([...API_ORDER]);
  expect(within(body[0]!).getAllByRole("cell").map(c => c.textContent)).toEqual(["Running", "10", "8", "0", "80 %"]);
  expect(body[1]).toHaveTextContent(/Quota exhausted – resets/);
  expect(body[3]).toHaveTextContent("Not connected");
  expect(within(body[4]!).getByRole("button", { name: "Configure Ahrefs" })).toBeVisible();
});

// spec: UI-070
it("opens the connection dialog of exactly the clicked source", async () => {
  const table = await open();
  fireEvent.click(within(table).getByRole("button", { name: "Configure Ahrefs" }));
  expect(useIntegrations.getState().dialog).toEqual({ kind: "ahrefs" });
  fireEvent.click(within(table).getByRole("button", { name: "Configure Moz" }));
  expect(useIntegrations.getState().dialog).toEqual({ kind: "moz" });
});

// spec: UI-070
it("lists final errors with URL, message and time on a click of the error count", async () => {
  const table = await open();
  fireEvent.click(within(table).getByRole("button", { name: "Show 2 errors of PageSpeed Insights" }));
  const list = await screen.findByRole("region", { name: "Errors of PageSpeed Insights" });
  expect(core.calls.find(c => c.cmd === "api.errors")?.args).toEqual({ kind: "psi", offset: 0, limit: 50 });
  expect((await within(list).findAllByRole("columnheader")).map(h => h.textContent)).toEqual(["URL", "Error message", "Time"]);
  expect(within(list).getByText("https://site.test/a/")).toBeVisible();
  expect(within(list).getByText("Quota exceeded")).toBeVisible();
});

// spec: UI-070
it("never shows credentials or tokens, not even when an error message contains one", async () => {
  const table = await open();
  fireEvent.click(within(table).getByRole("button", { name: "Show 2 errors of PageSpeed Insights" }));
  const list = await screen.findByRole("region", { name: "Errors of PageSpeed Insights" });
  await within(list).findByText("Quota exceeded");
  expect(list.textContent).not.toContain("AIzaSyDUMMYSECRET123");
  expect(list.textContent).toContain("api_key=••••");
  expect(redact("Authorization: Bearer abcdef123456")).not.toContain("abcdef123456");
  expect(redact("GET /x?access_token=abc.def&key=zzz1 failed")).toBe("GET /x?access_token=••••&key=•••• failed");
  expect(redact("HTTP code: 500")).toBe("HTTP code: 500");
  expect(table.textContent).not.toMatch(/token|secret|password/i);
});

// spec: UI-070
it("updates progress live when a tick reports a new sidebar version", async () => {
  const table = await open();
  expect(within(table).getByRole("row", { name: /Google Analytics 4/ })).toHaveTextContent("80 %");
  rows[0] = { ...rows[0]!, received: 10, completed: 10, running: false };
  act(() => useSession.setState({ sidebarVersion: 2 }));
  await within(table).findByText("100 %", { selector: "tr[data-api-kind='ga4'] td" });
  expect(within(table).getByRole("row", { name: /Google Analytics 4/ })).toHaveTextContent("Done");
});
