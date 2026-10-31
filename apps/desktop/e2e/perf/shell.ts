import "../../src/main";
import { client } from "../../src/ipc/client";
import { config } from "../../src/ipc/config";
import { crawls } from "../../src/ipc/crawls";
import { sidebar } from "../../src/ipc/sidebar";
import { crawl } from "../../src/ipc/crawl";
import { useSession } from "../../src/state/session";
const ticks: { at: number; phase?: string }[] = [];
const clicks: number[] = [];
client.subscribe("main", (event) => {
  if (event.kind === "tick")
    ticks.push({ at: performance.now(), phase: event.tick.crawl?.phase });
});
document.addEventListener(
  "click",
  () => {
    const start = performance.now();
    requestAnimationFrame(() => clicks.push(performance.now() - start));
  },
  true,
);
window.perfShell = {
  ticks,
  clicks,
  state: () => useSession.getState().crawl,
  async start(base: string) {
    const dto = await config.get();
    const root = dto.config as Record<string, Record<string, unknown>>;
    await config.set({
      ...dto,
      config: {
        ...root,
        speed: { ...root.speed, threads: 20, max_urls_per_sec: null },
        analysis: { ...root.analysis, auto_at_end: false },
      },
    });
    await crawl.start({
      source: { kind: "spider", url: base },
      scope: "domain",
    });
  },
  open: (path: string) => crawl.open({ kind: "path", path }),
  completeSidebars: async () => {
    await Promise.all([sidebar.overview({}), sidebar.issues({})]);
  },
  async importFixture(path: string) {
    const operation = await crawls.import({ paths: [path] });
    const progress: {
      done: number;
      total?: number | null;
      indicator: boolean;
    }[] = [];
    const deadline = performance.now() + 120_000;
    while (performance.now() < deadline) {
      const status = useSession.getState().ops[operation.opId];
      if (status) {
        progress.push({
          done: status.done,
          total: status.total,
          indicator: !!document.querySelector('[role="progressbar"]'),
        });
        if (status.state === "done") return { result: status.result, progress };
        if (status.state === "failed" || status.state === "cancelled")
          throw new Error(JSON.stringify(status));
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error("Import timed out");
  },
  stop: () => crawl.stop(),
};
declare global {
  interface Window {
    perfShell: {
      ticks: { at: number; phase?: string }[];
      clicks: number[];
      open(path: string): ReturnType<typeof crawl.open>;
      completeSidebars(): Promise<void>;
      importFixture(path: string): Promise<{
        result: unknown;
        progress: { done: number; total?: number | null; indicator: boolean }[];
      }>;
      state(): ReturnType<typeof useSession.getState>["crawl"];
      start(base: string): Promise<void>;
      stop(): Promise<void>;
    };
  }
}
