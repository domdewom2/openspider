import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import type { CrawlStatus } from "../../../../ipc/gen/CrawlStatus";
import type { OpStatus } from "../../../../ipc/gen/OpStatus";
import { useSession, type SessionState } from "../../../../state/session";
import fixture from "../custom-search/fixtures/fach.json";
import { previewDefs } from "./model";
import { useDraftPreview } from "./preview";
let core: MockCore;
const crawl = (crawlId: string): CrawlStatus => ({ crawlId, phase: "completed", mode: "spider", crawled: 10, queued: 0, discovered: 10, percent: 100, rate: 0, avgRate: 0, elapsedS: 1, pausedS: 0, activeThreads: 0, owner: { kind: "app", live: true, controllable: true } });
beforeEach(() => {
  vi.useFakeTimers(); vi.setSystemTime(new Date("2026-10-21T10:00:00Z"));
  core = new MockCore(); setClient(createClient(core.transport("main")));
  core.handlers.set("op.cancel", () => null);
  useSession.setState({ crawl: crawl("fixture-1"), countsVersion: 1, analysis: null, ops: {} });
});
afterEach(() => { cleanup(); vi.clearAllTimers(); vi.useRealTimers(); });
const advance = (ms = 400) => act(async () => { await vi.advanceTimersByTimeAsync(ms); });
const finish = (opId: string, urls: number) => act(() => useSession.setState(s => ({ ops: { ...s.ops, [opId]: { opId, labelKey: "preview", done: 1, state: "done", result: { counts: [{ id: "draft-0", urls }], unassigned: 0 } } satisfies OpStatus } })));
const cancelled = () => core.calls.filter(c => c.cmd === "op.cancel").map(c => c.args);

// spec: CRAWL-134
it("hides counters from an older priority list immediately and ignores its late result", async () => {
  let n = 0; core.handlers.set("segment.preview", () => ({ opId: `preview-${++n}` }));
  const defs = previewDefs(fixture.segments.list);
  const { result, rerender } = renderHook(({ defs }) => useDraftPreview(defs, true), { initialProps: { defs } });
  await advance(399); expect(n).toBe(0);
  await advance(1); expect(n).toBe(1); finish("preview-1", 10);
  expect(result.current.counts.get("draft-0")).toBe(10);
  rerender({ defs: previewDefs([...fixture.segments.list].reverse()) });
  expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(true);
  finish("preview-1", 999); expect(result.current.counts.size).toBe(0);
  await advance(); expect(n).toBe(2); finish("preview-2", 3);
  expect(result.current.counts.get("draft-0")).toBe(3);
  expect(cancelled()).toContainEqual({ opId: "preview-1" });
});

// spec: CRAWL-134
it.each<[string, Partial<SessionState>]>([
  ["new URLs", { countsVersion: 2 }],
  ["another crawl with the same counters", { crawl: crawl("fixture-2") }],
  ["completed analysis", { analysis: { running: false, steps: [{ step: "link_score", state: "done", stale: false }] } }],
  ["completed API fetch before the next counts tick", { ops: { api: { opId: "api", labelKey: "op-api-fetch", state: "done", done: 1 } } }],
])("refreshes unchanged fixture rules after %s and rejects the old result", async (_, patch) => {
  let n = 0; core.handlers.set("segment.preview", () => ({ opId: `preview-${++n}` }));
  const defs = previewDefs(fixture.segments.list);
  const { result } = renderHook(() => useDraftPreview(defs, true));
  await advance(); finish("preview-1", 10);
  expect(result.current.counts.get("draft-0")).toBe(10);
  act(() => useSession.setState(patch));
  expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(true);
  finish("preview-1", 999); expect(result.current.counts.size).toBe(0);
  await advance(); expect(n).toBe(2);
  expect(core.calls.filter(c => c.cmd === "segment.preview").map(c => c.args)).toEqual([{ segments: defs }, { segments: defs }]);
  finish("preview-2", 12); expect(result.current.counts.get("draft-0")).toBe(12);
  expect(cancelled()).toContainEqual({ opId: "preview-1" });
});

// spec: CRAWL-134
it("cancels a preview handle that arrives after the page was closed", async () => {
  let resolve!: (v: { opId: string }) => void;
  core.handlers.set("segment.preview", () => new Promise(r => { resolve = r; }));
  const { unmount } = renderHook(() => useDraftPreview(previewDefs(fixture.segments.list), true));
  await advance(); expect(resolve).toBeDefined(); unmount();
  await act(async () => resolve({ opId: "late-preview" }));
  expect(cancelled()).toContainEqual({ opId: "late-preview" });
});

// spec: CRAWL-134
it("cancels a delayed handle from the previous data revision and keeps the current result", async () => {
  let resolve!: (v: { opId: string }) => void;
  core.handlers.set("segment.preview", () => new Promise(r => { resolve = r; }));
  const { result } = renderHook(() => useDraftPreview(previewDefs(fixture.segments.list), true));
  await advance();
  act(() => useSession.setState({ countsVersion: 2 }));
  core.handlers.set("segment.preview", () => ({ opId: "current" }));
  await advance(); finish("current", 12);
  await act(async () => resolve({ opId: "stale" })); finish("stale", 999);
  expect(cancelled()).toContainEqual({ opId: "stale" });
  expect(result.current.counts.get("draft-0")).toBe(12);
});

// spec: CRAWL-134
it("clears counts on disable and starts a fresh request on re-enable without rules changing", async () => {
  let n = 0; core.handlers.set("segment.preview", () => ({ opId: `preview-${++n}` }));
  const { result, rerender, unmount } = renderHook(({ enabled }) => useDraftPreview(previewDefs(fixture.segments.list), enabled), { initialProps: { enabled: true } });
  await advance(); finish("preview-1", 10);
  rerender({ enabled: false }); expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(false);
  await advance(); expect(n).toBe(1);
  rerender({ enabled: true }); expect(result.current.counts.size).toBe(0);
  await advance(); expect(n).toBe(2);
  unmount(); expect(cancelled()).toContainEqual({ opId: "preview-2" });
});
