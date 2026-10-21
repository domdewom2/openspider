import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import type { OpStatus } from "../../../../ipc/gen/OpStatus";
import { useSession } from "../../../../state/session";
import fixture from "../custom-search/fixtures/fach.json";
import { previewDefs } from "./model";
import { useDraftPreview } from "./preview";
let core: MockCore;
beforeEach(() => {
  core = new MockCore(); setClient(createClient(core.transport("main")));
  core.handlers.set("op.cancel", () => null); useSession.setState({ ops: {} });
});
afterEach(cleanup);
const finish = (opId: string, urls: number) => act(() => useSession.setState(s => ({ ops: { ...s.ops, [opId]: { opId, labelKey: "preview", done: 1, state: "done", result: { counts: [{ id: "draft-0", urls }], unassigned: 0 } } satisfies OpStatus } })));

// spec: CRAWL-134
it("hides counters from an older priority list immediately and ignores its late result", async () => {
  let n = 0; core.handlers.set("segment.preview", () => ({ opId: `preview-${++n}` }));
  const defs = previewDefs(fixture.segments.list);
  const { result, rerender } = renderHook(({ defs }) => useDraftPreview(defs, true), { initialProps: { defs } });
  await waitFor(() => expect(n).toBe(1)); finish("preview-1", 10);
  expect(result.current.counts.get("draft-0")).toBe(10);
  rerender({ defs: previewDefs([...fixture.segments.list].reverse()) });
  expect(result.current.counts.size).toBe(0); expect(result.current.busy).toBe(true);
  finish("preview-1", 999); expect(result.current.counts.size).toBe(0);
  await waitFor(() => expect(n).toBe(2)); finish("preview-2", 3);
  expect(result.current.counts.get("draft-0")).toBe(3);
  expect(core.calls.filter(c => c.cmd === "op.cancel").map(c => c.args)).toContainEqual({ opId: "preview-1" });
});

// spec: CRAWL-134
it("cancels a preview handle that arrives after the page was closed", async () => {
  let resolve: (v: { opId: string }) => void = () => {};
  let started = false;
  core.handlers.set("segment.preview", () => { started = true; return new Promise(r => { resolve = r; }); });
  const { unmount } = renderHook(() => useDraftPreview(previewDefs(fixture.segments.list), true));
  await waitFor(() => expect(started).toBe(true)); unmount();
  await act(async () => resolve({ opId: "late-preview" }));
  await waitFor(() => expect(core.calls.find(c => c.cmd === "op.cancel" && (c.args as { opId: string }).opId === "late-preview")).toBeDefined());
});
