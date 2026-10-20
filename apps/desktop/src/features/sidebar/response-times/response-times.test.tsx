import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { ResponseTimesArgs } from "../../../ipc/gen/ResponseTimesArgs";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar-fach/fach-dto.json";
import { SidebarFrame } from "../frame/SidebarFrame";
import { useSidebarNavigation } from "../frame/state";
import { bucketLabel, bucketRows, bucketSearch } from "./model";

let core: MockCore;
beforeEach(async () => {
  core = new MockCore();
  core.handlers.set("sidebar.overview", () => ({ summary: [], tabs: [] }));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("sidebar.responseTimes", args => fixture.responseTimes[(args as ResponseTimesArgs).scope]);
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });

async function open() {
  render(<Providers><SidebarFrame /></Providers>);
  fireEvent.click(await screen.findByRole("tab", { name: "Response Times" }));
  return screen.findByRole("table", { name: "Response time distribution" });
}

// spec: UI-069
it("labels the eleven buckets 0–1 … 9–10 and 10+ and counts the fx-ui-slow delays into them", () => {
  const rows = bucketRows(fixture.responseTimes.all.buckets);
  expect(rows.map(r => r.label)).toEqual(["0–1", "1–2", "2–3", "3–4", "4–5", "5–6", "6–7", "7–8", "8–9", "9–10", "10+"]);
  const counts = Object.fromEntries(rows.map(r => [r.label, r.bucket.count]));
  expect(counts).toMatchObject({ "0–1": 2, "1–2": 1, "2–3": 1, "4–5": 1, "7–8": 1, "10+": 1 });
  expect(rows.reduce((s, r) => s + r.pct, 0)).toBeCloseTo(100, 0);
  expect(bucketLabel({ fromMs: 9000, toMs: 10000, count: 0 })).toBe("9–10");
});

// spec: UI-069
it("shows the specified columns, the hint, the no-response footer and a bar chart", async () => {
  const table = await open();
  expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Response Times", "Number of URLs", "% of Total"]);
  expect(within(table).getByRole("row", { name: /^0–1/ })).toHaveTextContent("28.6 %");
  expect(within(table).getByText("No response: 2")).toBeVisible();
  expect(screen.getByText(/until the document has been received completely, without rendering and without sub-resources/)).toBeVisible();
  expect(screen.getByRole("list", { name: "Response time chart" })).toBeVisible();
  expect(screen.getByRole("button", { name: "0–1 – 0–1 s: 2" })).toBeVisible();
});

// spec: UI-069
it("filters between all URLs and internal HTML only", async () => {
  const table = await open();
  fireEvent.change(screen.getByLabelText("URLs"), { target: { value: "internalHtml" } });
  await waitFor(() => expect(core.calls.some(c => c.cmd === "sidebar.responseTimes" && (c.args as ResponseTimesArgs).scope === "internalHtml")).toBe(true));
  await waitFor(() => expect(within(table).getByRole("row", { name: /^0–1/ })).toHaveTextContent("50.0 %"));
});

// spec: UI-069
it("sets the table search to Response Time ≥ a AND < b when a bucket is clicked", async () => {
  const table = await open();
  fireEvent.click(within(table).getByRole("button", { name: "1–2" }));
  expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Internal", search: { kind: "rules", tree: { children: [{ column: "Response Time", op: "greaterThan", value: "0.999" }, { column: "Response Time", op: "lessThan", value: "2.000" }] } } });
  fireEvent.click(screen.getByRole("button", { name: "10+ – 10+ s: 1" }));
  expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ search: { tree: { children: [{ op: "greaterThan", value: "9.999" }] } } });
  expect(bucketSearch({ fromMs: 0, toMs: 1000, count: 0 })).toMatchObject({ tree: { children: [{ op: "lessThan", value: "1.000" }] } });
});
