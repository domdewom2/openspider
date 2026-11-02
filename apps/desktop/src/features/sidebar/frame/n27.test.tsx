import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { IssueRow } from "../../../ipc/gen/IssueRow";
import type { Tick } from "../../../ipc/gen/Tick";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar/sidebar-dto.json";
import { Dashboard } from "../../dashboard/Dashboard";
import { SidebarFrame } from "./SidebarFrame";
import { useSidebarNavigation } from "./state";

// M2-N27 (UI-018): Dashboard „Top-Probleme“ and the Issues tab keep their last list while the core counts a new
// data version (useIssues after the pattern of useOverview); the Overview names the age of a running crawl's count.
let core: MockCore;
let issueCalls = 0;
let release: (rows: IssueRow[]) => void = () => undefined;
const issues = fixture.issues as IssueRow[];
const first = issues.filter(r => r.count > 0)[0]!;
beforeEach(async () => {
  core = new MockCore();
  issueCalls = 0;
  core.handlers.set("sidebar.overview", () => fixture.overview);
  // First answer at once; every later answer waits until the test releases it (a running count).
  core.handlers.set("sidebar.issues", () => ++issueCalls === 1 ? issues : new Promise<IssueRow[]>(resolve => { release = resolve; }));
  core.handlers.set("analysis.status", () => fixture.analysis);
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); });

const newVersion = (seq: number) => act(() => useSession.getState().applyTick({ ...tick, seq, countsVersion: seq, issuesVersion: seq, sidebarVersion: seq } as unknown as Tick));
const issueButton = () => screen.getByRole("button", { name: new RegExp(`${first.tab}: ${first.filter}`) });

// spec: UI-018
// spec: UI-062
it("Dashboard keeps its top issues visible while a new data version is counted and then shows the new list", async () => {
  render(<Providers><Dashboard /></Providers>);
  await screen.findByRole("button", { name: new RegExp(`${first.tab}: ${first.filter}`) });
  newVersion(50);
  await waitFor(() => expect(issueCalls).toBe(2), { timeout: 3_000 });
  // The follow-up request is running: the list stays, no "Loading…".
  expect(issueButton()).toBeVisible();
  expect(screen.queryByText("Loading…")).toBeNull();
  // Further versions while it runs do not cancel it or start another one.
  newVersion(51);
  newVersion(52);
  await act(async () => { await new Promise(resolve => setTimeout(resolve, 700)); });
  expect(issueCalls).toBe(2);
  expect(issueButton()).toBeVisible();
  await act(async () => release([{ ...first, count: 777 }]));
  expect(await screen.findByText("777")).toBeVisible();
  // The versions that arrived meanwhile are bundled into exactly one follow-up.
  await waitFor(() => expect(issueCalls).toBe(3), { timeout: 3_000 });
});

// spec: UI-018
// spec: UI-062
it("Issues tab keeps its list visible while a new data version is counted", async () => {
  render(<Providers><SidebarFrame /></Providers>);
  fireEvent.click(await screen.findByRole("tab", { name: "Issues" }));
  await screen.findByRole("button", { name: new RegExp(`${first.tab}: ${first.filter}`) });
  newVersion(60);
  await waitFor(() => expect(issueCalls).toBe(2), { timeout: 3_000 });
  expect(issueButton()).toBeVisible();
  expect(screen.queryByText("Loading…")).toBeNull();
  await act(async () => release([{ ...first, count: 4242 }]));
  expect(await screen.findByText("4,242")).toBeVisible();
});

// spec: UI-018
it("Overview names the age of a running crawl's shared count from 2 s on, and never for exact or paused states", async () => {
  let ageMs: number | undefined = 12_000;
  core.handlers.set("sidebar.overview", () => ({ ...fixture.overview, ...(ageMs === undefined ? {} : { ageMs }) }));
  render(<Providers><SidebarFrame /></Providers>);
  expect(await screen.findByText("As of 12 s ago")).toBeVisible();
  // Exact answer (no age): nothing shown.
  ageMs = undefined;
  newVersion(70);
  await waitFor(() => expect(screen.queryByText(/^As of/)).toBeNull(), { timeout: 3_000 });
  // A fresh count of the running crawl (below 2 s): nothing shown.
  ageMs = 1_500;
  newVersion(71);
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.overview").length).toBeGreaterThanOrEqual(3), { timeout: 3_000 });
  expect(screen.queryByText(/^As of/)).toBeNull();
  // Paused crawl: no age, even for an old answer.
  ageMs = 30_000;
  newVersion(72);
  expect(await screen.findByText("As of 30 s ago", {}, { timeout: 3_000 })).toBeVisible();
  act(() => useSession.setState({ crawl: { ...(tick.crawl as CrawlStatus), phase: "paused" } }));
  expect(screen.queryByText(/^As of/)).toBeNull();
});

// spec: UI-018
it("names the age in German", async () => {
  await setLanguage("de");
  core.handlers.set("sidebar.overview", () => ({ ...fixture.overview, ageMs: 12_400 }));
  render(<Providers><SidebarFrame /></Providers>);
  expect(await screen.findByText("Stand vor 12 s")).toBeVisible();
});
