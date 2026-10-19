import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Statusbar } from "./Statusbar";
import { setupHeader, tick } from "../header/fixtures/setup";
import { useSession, attachSession } from "../../state/session";
import { createEventBus } from "../../ipc/events";
import { setLanguage } from "../../i18n";
import { registerAction } from "../../ui/actions";

beforeEach(async () => { setupHeader(); await setLanguage("en"); vi.useFakeTimers(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
const publish = (seq: number, patch: Partial<NonNullable<typeof tick.crawl>>) => {
  act(() => { useSession.getState().applyTick({ ...tick, seq, crawl: { ...tick.crawl!, ...patch } }); vi.advanceTimersByTime(250); });
};

// spec: UI-013
it("renders the exact core rates, percent and ETA, including a decreasing percent", () => {
  useSession.getState().applyTick(tick); render(<Statusbar />);
  expect(screen.getByText("Current: 42.5 URL/s")).toBeVisible();
  expect(screen.getByText("Average: 40.1 URL/s")).toBeVisible();
  expect(screen.getByText("Time remaining: 0:20")).toBeVisible();
  expect(screen.getByText("Done 1,200 of 2,000 (60 %)")).toBeVisible();
  publish(2, { percent: 30, queued: 2800, rate: 0, etaS: null, phase: "paused" });
  expect(screen.getByText("Done 1,200 of 4,000 (30 %)")).toBeVisible();
  expect(screen.getByText("Current: 0.0 URL/s")).toBeVisible();
  expect(screen.getByText("Average: 40.1 URL/s")).toBeVisible();
  expect(screen.getByText("Time remaining: –")).toBeVisible();
  expect(document.querySelector('[aria-live="polite"]')).toBeNull();
});

// spec: CRAWL-130 [M2]
it("coalesces any number of ticks into at most four status paints per second", () => {
  render(<Statusbar />);
  act(() => { for (let i = 1; i <= 100; i++) useSession.getState().applyTick({ ...tick, seq: i, crawl: { ...tick.crawl!, crawled: i } }); });
  expect(screen.getByText(/Done 0 of 0/)).toBeVisible();
  act(() => { vi.advanceTimersByTime(249); });
  expect(screen.getByText(/Done 0 of 0/)).toBeVisible();
  act(() => { vi.advanceTimersByTime(1); });
  expect(screen.getByText(/Done 100 of 900/)).toBeVisible();
  act(() => useSession.getState().applyTick({ ...tick, seq: 99 }));
  act(() => { vi.advanceTimersByTime(250); });
  expect(screen.getByText(/Done 100 of 900/)).toBeVisible();
});

// spec: CRAWL-097
it("retains per-host throttle status after notices are consumed and clears it on recovery", () => {
  const bus = createEventBus(); const off = attachSession(bus);
  render(<Statusbar />);
  act(() => {
    bus.emit({ kind: "notice", notice: { level: "warning", code: "crawl-throttled", args: { host: "site.test" } } });
    useSession.getState().clearNotices(); vi.advanceTimersByTime(250);
  });
  expect(screen.getByText("Throttled")).toBeVisible();
  act(() => {
    bus.emit({ kind: "notice", notice: { level: "info", code: "crawl-throttle-cleared", args: { host: "site.test" } } });
    vi.advanceTimersByTime(250);
  });
  expect(screen.queryByText("Throttled")).toBeNull();
  off(); expect(bus.listenerCount()).toBe(0);
});

// spec: CRAWL-129
it("uses the core memory stages without deriving UI thresholds", () => {
  useSession.getState().applyTick(tick); render(<Statusbar />);
  expect(screen.getByText("Memory: 800 MB / 1.00 GB")).not.toHaveClass("os-status-warning");
  publish(2, { memoryStage: "warning", memoryUsedBytes: 100 });
  expect(screen.getByText(/Memory: 100 B/)).toHaveClass("os-status-warning");
  publish(3, { memoryStage: "paused" });
  expect(screen.getByText(/Paused due to low memory/)).toHaveClass("os-status-danger");
});

// spec: UI-013
it("shows limits, named analysis steps, statistics, and the always visible MCP menu action", () => {
  const open = vi.fn(); const off = registerAction("mcp.menu", open);
  useSession.getState().applyTick({ ...tick, crawl: { ...tick.crawl!, phase: "completed", endReason: "limit_reached" }, analysis: { running: true, current: "link_score", percent: 37, steps: [{ step: "link_score", state: "running", stale: false }] } });
  render(<Statusbar />);
  expect(screen.getByText("Link Score 1/1 · 37 %")).toBeVisible();
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "37");
  expect(screen.getByText(/Limit reached/)).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "MCP: connected (1 client)" }));
  expect(open).toHaveBeenCalledOnce(); off();
  fireEvent.click(screen.getByText("Statistics"));
  expect(screen.getByText("Average response time: 0.231 s")).toBeVisible();
});
