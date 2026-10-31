import { writeFileSync } from "node:fs";
import { Profiler } from "react";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Header } from "./Header";
import { setupHeader, tick } from "./fixtures/setup";
import { useSession } from "../../state/session";
import { setClient } from "../../ipc/client";

afterEach(() => { cleanup(); setClient(null); vi.useRealTimers(); });

// spec: UI-143
it("profiles numeric crawl ticks independently from header controls (fixture tick-running)", async () => {
  setupHeader();
  useSession.getState().applyTick(tick);
  const commits: { phase: string; ms: number }[] = [];
  render(<Profiler id="header" onRender={(_id, phase, ms) => commits.push({ phase, ms })}><Header /></Profiler>);
  await act(async () => {});
  vi.useFakeTimers();
  act(() => vi.advanceTimersByTime(1000));
  commits.length = 0;
  for (let i = 1; i <= 10; i++) {
    const before = commits.length;
    act(() => useSession.getState().applyTick({ ...tick, seq: tick.seq + i, crawl: { ...tick.crawl!, crawled: i * 100, rate: 200, elapsedS: i / 2 } }));
    expect(commits).toHaveLength(before); // controls did not render on numeric telemetry
    act(() => vi.advanceTimersByTime(500));
    expect(commits.length - before).toBeLessThanOrEqual(2); // status-card/RAC progress update
  }
  if (process.env.N23_PROFILE_OUTPUT) writeFileSync(process.env.N23_PROFILE_OUTPUT, JSON.stringify({ commits: commits.length, ms: commits.reduce((sum, c) => sum + c.ms, 0) }));
  expect(commits.length).toBeLessThanOrEqual(11);
  expect(useSession.getState().crawl?.crawled).toBe(1000);
});
