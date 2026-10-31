import type { ViewDataSource } from "../../src/data/ViewDataSource";
import type { ViewHandle } from "../../src/ipc/gen/ViewHandle";
import type { ViewSpec } from "../../src/ipc/gen/ViewSpec";
import type { view as View } from "../../src/ipc/view";
import type { client as Client } from "../../src/ipc/client";
import { paint } from "../../src/grid/recorder";
import type { settled as Settled } from "./settled";
export function installPerf({
  api,
  source,
  handle,
  view,
  client,
  settled,
  openStart,
}: {
  api: Window["perfGrid"];
  source: ViewDataSource;
  handle: ViewHandle;
  view: typeof View;
  client: typeof Client;
  settled: typeof Settled;
  openStart: number;
}) {
  const tab = (extra: object = {}): ViewSpec => ({
    kind: "tab",
    tab: "Internal",
    filter: "All",
    sort: [],
    ...extra,
  });
  const columns = handle.columns.slice(0, 20).map((c) => c.key);
  const errors = api.errors;
  let violations = 0;
  const ticks: number[] = [];
  client.subscribe("main", (e) => {
    if (e.kind === "tick") ticks.push(performance.now());
  });
  const run = async (spec: ViewSpec) => {
    ({ source, handle } = api.current());
    performance.mark("u31-operation-start");
    const h = await settled(await view.update({ viewId: handle.viewId, spec }));
    handle = h;
    await source.onViewChanged(h.version, h.total ?? 0);
    await source.ensureRange(0, 30);
    await paint();
    performance.mark("u31-first-rows");
    const duration = performance.measure(
      "u31-operation",
      "u31-operation-start",
      "u31-first-rows",
    ).duration;
    performance.clearMarks("u31-operation-start");
    performance.clearMarks("u31-first-rows");
    performance.clearMeasures("u31-operation");
    return duration;
  };
  window.uiPerf = {
    ready: api.ready,
    snapshot: api.snapshot,
    errors,
    async openMs() {
      while (!api.ready()) await paint();
      await paint();
      return performance.now() - openStart;
    },
    async ipc() {
      const samples = [];
      // Build the exact 20-column view before warmup/timing. Measuring a different
      // projection on the 65-column grid view also includes projection rebuilds.
      const ipcHandle = await settled(
        await view.open({ spec: tab(), columns }),
      );
      try {
        for (let i = 0; i < 55; i++) {
          const start = performance.now();
          const result = await view.rows({
            viewId: ipcHandle.viewId,
            version: ipcHandle.version,
            offset: (i * 200) % Math.max(200, source.total - 200),
            limit: 200,
            columns,
          });
          const ms = performance.now() - start;
          if (result.cells.length !== 200 || result.columns.length !== 20)
            throw new Error("Expected 200 × 20 response");
          if (i >= 5)
            samples.push({
              ms,
              bytes: new TextEncoder().encode(JSON.stringify(result))
                .byteLength,
            });
        }
        return { samples, columns, rows: 200 };
      } finally {
        await view.close({ viewId: ipcHandle.viewId });
      }
    },
    record: api.record,
    jump: (row: number) => api.jump(row),
    async operations() {
      const result: Record<string, number> = {};
      const measure = async (
        name: string,
        operation: () => Promise<number>,
      ) => {
        try {
          result[name] = await operation();
        } catch (error) {
          result[name] = NaN;
          if (errors.length < 100) errors.push(`${name}: ${String(error)}`);
        }
      };
      await measure("tabMs", async () => {
        const start = performance.now();
        await api.switchTab("Response Codes");
        return performance.now() - start;
      });
      await measure("tabReturnMs", async () => {
        const start = performance.now();
        await api.switchTab("Internal");
        return performance.now() - start;
      });
      await measure("filterMs", () => run(tab({ filter: "HTML" })));
      await measure("numericMs", () =>
        run(tab({ sort: [{ column: "Status Code", dir: "asc" }] })),
      );
      await measure("reverseMs", () =>
        run(tab({ sort: [{ column: "Status Code", dir: "desc" }] })),
      );
      await measure("textMs", () =>
        run(tab({ sort: [{ column: "Title 1", dir: "asc" }] })),
      );
      await measure("substringMs", () =>
        run(tab({ search: { kind: "simple", text: "0001" } })),
      );
      await measure("regexMs", () =>
        run(
          tab({
            search: {
              kind: "rules",
              caseSensitive: false,
              tree: {
                kind: "cond",
                column: "Address",
                op: "regex",
                value: "000[12]",
              },
            },
          }),
        ),
      );
      return result;
    },
    async sidebars() {
      const times: Record<string, number> = {};
      for (const [cmd, args] of [
        ["sidebar.overview", {}],
        ["sidebar.issues", {}],
        [
          "sidebar.siteStructure",
          { view: "responseCodes", offset: 0, limit: 200 },
        ],
        ["sidebar.responseTimes", { scope: "all" }],
      ] as const) {
        const start = performance.now();
        await client.invoke(cmd, args);
        times[cmd] = performance.now() - start;
      }
      return times;
    },
    async action(seed: number) {
      const row = (seed >>> 0) % Math.max(1, api.snapshot().total);
      if ((seed & 15) === 0) {
        try {
          await api.switchTab(
            (seed & 16) === 0 ? "Response Codes" : "Internal",
          );
        } catch (error) {
          violations++;
          if (errors.length < 100) errors.push(String(error));
        }
      }
      const result = await api.jump(
        row,
        (seed >>> 8) % Math.min(65, api.snapshot().columns),
      );
      if (
        !result.contentReady ||
        result.cachedRows > 20_000 ||
        result.domRows >= 300
      ) {
        violations++;
        if (errors.length < 100) errors.push(JSON.stringify(result));
      }
      return result;
    },
    ticks: () => ticks.slice(),
    violations: () => violations,
  };
}
declare global {
  interface Window {
    uiPerf: {
      ready(): boolean;
      snapshot: Window["perfGrid"]["snapshot"];
      errors: string[];
      openMs(): Promise<number>;
      ipc(): Promise<{
        samples: { ms: number; bytes: number }[];
        columns: string[];
        rows: number;
      }>;
      record: Window["perfGrid"]["record"];
      jump(row: number): ReturnType<Window["perfGrid"]["jump"]>;
      operations(): Promise<Record<string, number>>;
      sidebars(): Promise<Record<string, number>>;
      violations(): number;
      action(seed: number): ReturnType<Window["perfGrid"]["jump"]>;
      ticks(): number[];
    };
  }
}
