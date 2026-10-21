import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import type { CrawlStatus } from "../../../../ipc/gen/CrawlStatus";
import type { OpStatus } from "../../../../ipc/gen/OpStatus";
import { useSession, type SessionState } from "../../../../state/session";
import fixture from "../custom-search/fixtures/fach.json";
import { previewDefs } from "./model";
import { previewRevision, useDraftPreview } from "./preview";
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
const patch = (state: Partial<SessionState>) => act(() => useSession.setState(state));
const cancelled = () => core.calls.filter(c => c.cmd === "op.cancel").map(c => c.args);
const requests = () => core.calls.filter(c => c.cmd === "segment.preview");
const defs = () => previewDefs(fixture.segments.list);
const numberedHandles = () => { let n = 0; core.handlers.set("segment.preview", () => ({ opId: `preview-${++n}` })); };

// spec: CRAWL-134
it("hides counters from an older priority list immediately and ignores its late result", async () => {
  numberedHandles();
  const { result, rerender } = renderHook(({ rules }) => useDraftPreview(rules, true), { initialProps: { rules: defs() } });
  await advance(399); expect(requests()).toHaveLength(0);
  await advance(1); expect(requests()).toHaveLength(1); finish("preview-1", 10);
  expect(result.current.counts.get("draft-0")).toBe(10);
  rerender({ rules: previewDefs([...fixture.segments.list].reverse()) });
  expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(true);
  finish("preview-1", 999); expect(result.current.counts.size).toBe(0);
  await advance(); finish("preview-2", 3);
  expect(result.current.counts.get("draft-0")).toBe(3);
});

// spec: CRAWL-134
it.each<[string, Partial<SessionState>]>([
  ["new URLs", { countsVersion: 2 }],
  ["completed analysis", { analysis: { running: false, steps: [{ step: "link_score", state: "done", stale: false }] } }],
  ["completed API fetch before the next counts tick", { ops: { api: { opId: "api", labelKey: "op-api-fetch", state: "done", done: 1 } } }],
])("refreshes unchanged fixture rules after %s and labels the retained result", async (_, state) => {
  numberedHandles();
  const { result } = renderHook(() => useDraftPreview(defs(), true));
  await advance(); finish("preview-1", 10);
  const firstRevision = result.current.resultRevision;
  expect(result.current.stale).toBe(false);
  patch(state);
  expect(result.current.counts.get("draft-0")).toBe(10);
  expect(result.current.resultRevision).toBe(firstRevision);
  expect(result.current.stale).toBe(true); expect(result.current.busy).toBe(true);
  // Duplicate/late ticks for a completed operation must not overwrite its published result.
  finish("preview-1", 999); expect(result.current.counts.get("draft-0")).toBe(10);
  await advance(); expect(requests().map(c => c.args)).toEqual([{ segments: defs() }, { segments: defs() }]);
  finish("preview-2", 12); expect(result.current.counts.get("draft-0")).toBe(12);
  expect(result.current.resultRevision).toBe(previewRevision(useSession.getState()));
  expect(result.current.stale).toBe(false); expect(result.current.busy).toBe(false);
  expect(cancelled()).toEqual([]);
});

// spec: CRAWL-134
it("bounds the first wait and serializes delayed previews during continuous 250-ms revisions", async () => {
  let resolve!: (v: { opId: string }) => void;
  core.handlers.set("segment.preview", () => new Promise(r => { resolve = r; }));
  const { result } = renderHook(() => useDraftPreview(defs(), true));
  await advance(250); patch({ countsVersion: 2 });
  await advance(149); expect(requests()).toHaveLength(0);
  await advance(1); expect(requests()).toHaveLength(1); // t=400, despite the t=250 tick
  const firstRevision = previewRevision(useSession.getState());
  for (let revision = 3; revision <= 8; revision++) {
    await advance(revision === 3 ? 100 : 250); patch({ countsVersion: revision });
    expect(requests()).toHaveLength(1); expect(cancelled()).toEqual([]);
  }
  // Even handle delivery can be slower than the continuous data feed.
  await act(async () => resolve({ opId: "preview-1" }));
  await advance(250); patch({ countsVersion: 9 });
  core.handlers.set("segment.preview", () => ({ opId: "preview-2" }));
  await act(async () => { finish("preview-1", 10); });
  expect(result.current.counts.get("draft-0")).toBe(10);
  expect(result.current.resultRevision).toBe(firstRevision);
  expect(result.current.countsVersion).toBe(2);
  expect(result.current.stale).toBe(true); expect(result.current.busy).toBe(true);
  expect(requests()).toHaveLength(2); // newest revision starts as soon as the first finishes
  for (let revision = 10; revision <= 13; revision++) {
    await advance(250); patch({ countsVersion: revision });
    expect(requests()).toHaveLength(2);
    expect(result.current.counts.get("draft-0")).toBe(10);
  }
  core.handlers.set("segment.preview", () => ({ opId: "preview-3" }));
  await act(async () => { finish("preview-2", 12); });
  expect(result.current.counts.get("draft-0")).toBe(12);
  expect(result.current.countsVersion).toBe(9); expect(result.current.stale).toBe(true);
  expect(requests()).toHaveLength(3);
  finish("preview-3", 14);
  expect(result.current.counts.get("draft-0")).toBe(14);
  expect(result.current.countsVersion).toBe(13); expect(result.current.stale).toBe(false);
  expect(result.current.busy).toBe(false); expect(cancelled()).toEqual([]);
  await advance(2000); expect(requests()).toHaveLength(3);
});

// spec: CRAWL-134
it("does not extend the refresh window with subsequent data ticks", async () => {
  numberedHandles();
  const { result } = renderHook(() => useDraftPreview(defs(), true));
  await advance(); finish("preview-1", 10);
  patch({ countsVersion: 2 }); await advance(250); patch({ countsVersion: 3 });
  await advance(149); expect(requests()).toHaveLength(1);
  await advance(1); expect(requests()).toHaveLength(2);
  finish("preview-2", 12);
  expect(result.current.countsVersion).toBe(3); expect(result.current.stale).toBe(false);
});

// spec: CRAWL-134
it.each(["rules", "crawl"])("cancels an in-flight preview on %s changes and ignores late completion", async change => {
  numberedHandles();
  const { result, rerender } = renderHook(({ rules }) => useDraftPreview(rules, true), { initialProps: { rules: defs() } });
  await advance();
  if (change === "rules") rerender({ rules: [...defs()].reverse() });
  else patch({ crawl: crawl("fixture-2") });
  expect(cancelled()).toContainEqual({ opId: "preview-1" });
  finish("preview-1", 999); expect(result.current.counts.size).toBe(0);
  await advance(); finish("preview-2", 12);
  expect(result.current.counts.get("draft-0")).toBe(12);
});

// spec: CRAWL-134
it.each(["close", "rules", "crawl"])("cancels a delayed handle after %s invalidates its context", async change => {
  let resolve!: (v: { opId: string }) => void;
  core.handlers.set("segment.preview", () => new Promise(r => { resolve = r; }));
  const { result, rerender, unmount } = renderHook(({ rules }) => useDraftPreview(rules, true), { initialProps: { rules: defs() } });
  await advance();
  if (change === "close") unmount();
  else if (change === "rules") rerender({ rules: [...defs()].reverse() });
  else patch({ crawl: crawl("fixture-2") });
  core.handlers.set("segment.preview", () => ({ opId: "current" }));
  await advance();
  if (change !== "close") finish("current", 12);
  await act(async () => resolve({ opId: "late-preview" })); finish("late-preview", 999);
  expect(cancelled()).toContainEqual({ opId: "late-preview" });
  if (change !== "close") expect(result.current.counts.get("draft-0")).toBe(12);
});

// spec: CRAWL-134
it("accepts a completion tick that preceded the operation handle", async () => {
  let resolve!: (v: { opId: string }) => void;
  core.handlers.set("segment.preview", () => new Promise(r => { resolve = r; }));
  const { result } = renderHook(() => useDraftPreview(defs(), true));
  await advance(); finish("early", 10);
  await act(async () => resolve({ opId: "early" }));
  expect(result.current.counts.get("draft-0")).toBe(10); expect(result.current.busy).toBe(false);
});

// spec: CRAWL-134
it.each(["failed", "cancelled", "rejected"])("continues with the newest revision after a %s request", async state => {
  if (state === "rejected") core.handlers.set("segment.preview", () => { throw new Error("fixture failure"); });
  else numberedHandles();
  const { result } = renderHook(() => useDraftPreview(defs(), true));
  await advance();
  if (state !== "rejected") patch({ ops: { "preview-1": { opId: "preview-1", labelKey: "preview", done: 0, state: state as "failed" | "cancelled" } } });
  expect(result.current.error).toBeDefined(); expect(result.current.busy).toBe(false);
  core.handlers.set("segment.preview", () => ({ opId: "current" }));
  patch({ countsVersion: 2 }); await advance(); finish("current", 12);
  expect(result.current.counts.get("draft-0")).toBe(12);
  expect(result.current.error).toBeUndefined(); expect(result.current.busy).toBe(false);
});

// spec: CRAWL-134
it("clears counts on disable and starts a fresh request on re-enable without rules changing", async () => {
  numberedHandles();
  const { result, rerender, unmount } = renderHook(({ enabled }) => useDraftPreview(defs(), enabled), { initialProps: { enabled: true } });
  await advance(); finish("preview-1", 10);
  rerender({ enabled: false }); expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(false);
  await advance(); expect(requests()).toHaveLength(1);
  rerender({ enabled: true }); expect(result.current.counts.size).toBe(0);
  await advance(); expect(requests()).toHaveLength(2);
  unmount(); expect(cancelled()).toContainEqual({ opId: "preview-2" });
});
