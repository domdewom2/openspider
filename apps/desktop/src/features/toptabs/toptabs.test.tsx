import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { eventBus } from "../../ipc/events";
import { uistate } from "../../ipc/uistate";
import { useTabs } from "../../state/tabs";
import { useSession } from "../../state/session";
import { setLanguage } from "../../i18n";
import type { CrawlStatus } from "../../ipc/gen/CrawlStatus";
import { fixtureTabs, tableFixture } from "../table/fixtures/core";
import { TopTabs } from "./TopTabs";

let fx: ReturnType<typeof tableFixture>;
beforeEach(async () => {
  fx = tableFixture(); setClient(createClient(fx.core.transport()));
  useTabs.setState({ catalog: [], order: [], hidden: [], active: "Internal", queries: {}, loaded: false, focusId: undefined, focusRequest: 0, error: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); });

// spec: UI-004 [P1]
it("loads only catalog tabs, persists reordering and hiding, and resets to catalog defaults", async () => {
  fx.core.uiState.state = { unrelated: { width: 320 } };
  await useTabs.getState().load();
  expect(useTabs.getState().order).toEqual(fixtureTabs.map(t => t.id));
  await useTabs.getState().reorder(["Images", "Internal", "External", "AMP"]);
  await useTabs.getState().show("AMP", false); await useTabs.getState().show("Internal", false);
  expect(useTabs.getState().hidden).toEqual(["AMP"]);
  useTabs.setState({ order: [], hidden: [] }); await useTabs.getState().load();
  expect(useTabs.getState().order[0]).toBe("Images"); expect(useTabs.getState().hidden).toEqual(["AMP"]);
  expect((await uistate.load()).state).toMatchObject({ unrelated: { width: 320 } });
  await useTabs.getState().reset();
  expect(useTabs.getState().order).toEqual(fixtureTabs.map(t => t.id)); expect(useTabs.getState().hidden).toEqual([]);
});

// spec: UI-006
it("retains each tab's last filter and sort, and accepts navigation from Overview", async () => {
  await useTabs.getState().load(); const s = useTabs.getState();
  s.select("Internal", "Client Error (4xx)"); s.query({ sort: [{ column: "Address", dir: "desc" }] });
  s.select("Images", "Unlinked"); s.select("Internal");
  expect(useTabs.getState().queries.Internal).toMatchObject({ filter: "Client Error (4xx)", sort: [{ column: "Address", dir: "desc" }] });
  s.select("Internal", "All"); expect(useTabs.getState().queries.Internal.filter).toBe("All");
});

// spec: UI-004 [P1]
it("offers checkboxes for every tab, protects Internal, and reopens a hidden tab", async () => {
  render(<TopTabs />); await screen.findByRole("tab", { name: "Internal" });
  expect(screen.getByRole("checkbox", { name: "Internal" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "AMP" }));
  await waitFor(() => expect(useTabs.getState().hidden).toContain("AMP"));
  expect(screen.queryByRole("tab", { name: "AMP" })).toBeNull();
  fireEvent.click(screen.getByRole("checkbox", { name: "AMP" }));
  await screen.findByRole("tab", { name: "AMP" });
  fireEvent.click(screen.getByRole("button", { name: "Move Images up" }));
  await waitFor(() => expect(useTabs.getState().order).toEqual(["Internal", "Images", "External", "AMP"]));
});

// spec: UI-090
it("registers focusUrl navigation, checks crawl identity, and reveals and resets the target tab", async () => {
  const before = eventBus.listenerCount("focusUrl");
  const mounted = render(<TopTabs />); await screen.findByRole("tab", { name: "Internal" });
  useSession.setState({ crawl: { crawlId: "crawl-1" } as CrawlStatus });
  await act(async () => { await useTabs.getState().show("External", false); useTabs.getState().select("External", "Client Error (4xx)"); useTabs.getState().query({ search: "missing" }); });
  act(() => eventBus.emit({ kind: "focusUrl", crawlId: "other-crawl", urlId: 2, isInternal: false }));
  expect(useTabs.getState().focusId).toBeUndefined();
  act(() => eventBus.emit({ kind: "focusUrl", crawlId: "crawl-1", urlId: 2, isInternal: false }));
  expect(useTabs.getState()).toMatchObject({ active: "External", focusId: 2, queries: { External: { filter: "All", search: "" } } });
  expect(useTabs.getState().hidden).not.toContain("External");
  mounted.unmount(); expect(eventBus.listenerCount("focusUrl")).toBe(before);
});
