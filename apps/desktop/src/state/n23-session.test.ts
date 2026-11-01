import { expect, it } from "vitest";
import { tick, setupHeader } from "../features/header/fixtures/setup";
import { setClient } from "../ipc/client";
import { useSession } from "./session";

// spec: UI-143
it("numeric fixture ticks preserve unrelated selector snapshots and apply view diffs", () => {
  setupHeader();
  try {
    useSession.getState().applyTick({ ...tick, views: [{ viewId: "v", version: 2, total: 10 }], ops: [] });
    const before = useSession.getState();
    useSession.getState().applyTick({ ...tick, seq: tick.seq + 1, notices: [], views: [], ops: [], crawl: { ...tick.crawl!, crawled: 200 } });
    const next = useSession.getState();
    for (const key of ["views", "ops", "throttled", "crawlDelays", "notices"] as const) expect(next[key]).toBe(before[key]);
    expect(next.crawl?.crawled).toBe(200);
    useSession.getState().applyTick({ ...tick, seq: tick.seq + 2, views: [{ viewId: "v", version: 2, total: 10 }], ops: [], notices: [] });
    expect(useSession.getState().views).toBe(before.views);
    useSession.getState().applyTick({ ...tick, seq: tick.seq + 3, views: [{ viewId: "v", version: 3, total: 20 }], ops: [], notices: [] });
    expect(useSession.getState().views.v).toEqual({ viewId: "v", version: 3, total: 20 });
    expect(before.views.v?.total).toBe(10);
    useSession.getState().applyTick({ ...tick, seq: tick.seq + 4, notices: [{ level: "info", code: "crawl-throttled", args: { host: "fixture.test" } }] });
    expect(useSession.getState().throttled["fixture.test"]).toBe(true);
    expect(before.throttled).toEqual({});
  } finally { setClient(null); }
});
