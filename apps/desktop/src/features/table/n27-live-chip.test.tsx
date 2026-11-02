import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DataGridProps } from "../../grid/DataGrid";
import { createClient, setClient } from "../../ipc/client";
import type { CrawlStatus } from "../../ipc/gen/CrawlStatus";
import { useTabs } from "../../state/tabs";
import { useColumns } from "../../state/columns";
import { useSession } from "../../state/session";
import { setLanguage } from "../../i18n";
import { flushUiState } from "../../state/uistate";
import tick from "../../ipc/__fixtures__/Tick.json";
import { UpperTable } from "./UpperTable";
import { fixtureTabs, tableFixture } from "./fixtures/core";

const capture = vi.hoisted(() => ({ grid: undefined as DataGridProps | undefined }));
vi.mock("../../grid/DataGrid", async original => {
  const lib = await original<typeof import("../../grid/DataGrid")>();
  return { ...lib, DataGrid: (props: DataGridProps) => { capture.grid = props; return <div data-testid="grid" />; } };
});
let fx: ReturnType<typeof tableFixture>;
beforeEach(async () => {
  fx = tableFixture(); setClient(createClient(fx.core.transport()));
  // The shared count lags behind: it still knows one URL while the view already holds three.
  fx.core.handlers.set("catalog.filterCounts", () => ({ version: 1, filters: fixtureTabs.flatMap(tab => tab.filters.map(filter => ({ tab: tab.id, filter: { ...filter, count: filter.id === "All" ? 1 : filter.count } }))) }));
  useTabs.setState({ catalog: fixtureTabs, order: fixtureTabs.map(t => t.id), active: "Internal", hidden: [], queries: {}, loaded: true, focusId: undefined, focusRequest: 0, error: null });
  useColumns.setState({ metadata: {}, byTab: {} });
  useSession.setState({ ops: {}, crawl: { ...(tick.crawl as CrawlStatus), phase: "running" } }); capture.grid = undefined;
  await setLanguage("en");
});
afterEach(async () => { await flushUiState(); cleanup(); setClient(null); vi.restoreAllMocks(); });

// spec: UI-018
it("while the crawl runs, the active filter's chip and the footer follow the live view total, not the throttled shared count", async () => {
  render(<UpperTable />);
  await screen.findByTestId("grid");
  expect(await screen.findByRole("button", { name: "All (3)" })).toBeVisible();
  expect(screen.getByRole("status")).toHaveTextContent("Showing: 3 of 3");
  // Without a running crawl the shared count is exact and shown as it is.
  act(() => useSession.setState({ crawl: { ...(tick.crawl as CrawlStatus), phase: "paused" } }));
  await waitFor(() => expect(screen.getByRole("button", { name: "All (1)" })).toBeVisible());
});
