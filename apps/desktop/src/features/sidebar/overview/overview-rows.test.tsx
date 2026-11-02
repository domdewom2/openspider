import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { Tick } from "../../../ipc/gen/Tick";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar/sidebar-dto.json";
import { SidebarFrame } from "../frame/SidebarFrame";
import { useSidebarNavigation } from "../frame/state";

// M2-FIX-LIVE: every Overview filter row renders its FilterStatus exactly once per render, so the wrapper counts row renders.
const rows = vi.hoisted(() => ({ rendered: 0 }));
vi.mock("../analysis/FilterStatus", async importOriginal => {
  const actual = await importOriginal<typeof import("../analysis/FilterStatus")>();
  return { ...actual, FilterStatus: (props: Parameters<typeof actual.FilterStatus>[0]) => { rows.rendered++; return actual.FilterStatus(props); } };
});

let core: MockCore;
let answer: OverviewDto;
const base = fixture.overview as unknown as OverviewDto;
const filterCount = base.tabs.reduce((n, tab) => n + tab.filters.length, 0);
beforeEach(async () => {
  core = new MockCore();
  answer = { ...structuredClone(base), ageMs: 2_500 };
  // Every answer is a fresh object graph, as it arrives from the transport.
  core.handlers.set("sidebar.overview", () => structuredClone(answer));
  core.handlers.set("analysis.status", () => fixture.analysis);
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("en");
  rows.rendered = 0;
});
afterEach(() => { cleanup(); setClient(null); });

const newVersion = (seq: number) => act(() => useSession.getState().applyTick({ ...tick, seq, countsVersion: seq, issuesVersion: seq, sidebarVersion: seq } as unknown as Tick));
const overviewCalls = () => core.calls.filter(c => c.cmd === "sidebar.overview").length;

// spec: UI-145
// spec: UI-018
// spec: UI-143
it("a live Overview answer renders only the filter rows whose values changed", async () => {
  render(<Providers><SidebarFrame /></Providers>);
  expect(await screen.findByText("As of 3 s ago")).toBeVisible();
  await waitFor(() => expect(rows.rendered).toBeGreaterThanOrEqual(filterCount));
  const initial = rows.rendered;
  expect(initial).toBe(filterCount);

  // The throttled count answers again with the same counts and a new age: no filter row renders again.
  answer = { ...answer, ageMs: 4_800 };
  newVersion(10);
  expect(await screen.findByText("As of 5 s ago")).toBeVisible();
  expect(overviewCalls()).toBe(2);
  expect(rows.rendered).toBe(initial);

  // The next count changes one filter: only that row renders again, and it shows the new value.
  const changed = structuredClone(answer);
  const target = changed.tabs[0]!.filters[1]!;
  target.count = 4_242; target.pct = 42;
  answer = { ...changed, ageMs: 2_100 };
  newVersion(11);
  expect(await screen.findByText("4,242")).toBeVisible();
  expect(rows.rendered).toBe(initial + 1);
});
