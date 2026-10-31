// Diagnostic-only adapter for locked Playwright 1.64.1. Its in-process bridge
// exposes the bundled WebKit Inspector protocol (no public CDP API for WebKit).
import type { Page } from "@playwright/test";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

type Listener = (event: unknown) => void;
interface Inspector {
  send(method: string, params?: Record<string, unknown>): Promise<Record<string, unknown>>;
  on(event: string, listener: Listener): void;
  removeListener(event: string, listener: Listener): void;
}

export async function startWebKitCapture(page: Page, out: string, label: string): Promise<(heap?: boolean) => Promise<void>> {
  const internal = page as unknown as { _connection: { toImpl(page: Page): { delegate: { _session: Inspector } } } };
  const session = internal._connection.toImpl(page).delegate._session;
  if (!session) throw new Error("N23: locked WebKit Inspector adapter unavailable");
  const timeline: unknown[] = [], scripts: unknown[] = [], cpu: unknown[] = [];
  const timelineListener: Listener = event => timeline.push(event);
  const scriptListener: Listener = event => scripts.push(event);
  const cpuListener: Listener = event => cpu.push(event);
  session.on("Timeline.eventRecorded", timelineListener);
  session.on("ScriptProfiler.trackingComplete", scriptListener);
  session.on("CPUProfiler.trackingUpdate", cpuListener);
  await session.send("Timeline.enable");
  await session.send("Timeline.start", { maxCallStackDepth: 8 });
  await session.send("ScriptProfiler.startTracking", { includeSamples: true });
  let cpuError: string | null = null;
  try { await session.send("CPUProfiler.startTracking"); }
  catch (error) { cpuError = String(error); }
  return async (heap = false) => {
    await session.send("ScriptProfiler.stopTracking");
    await session.send("Timeline.stop");
    if (!cpuError) await session.send("CPUProfiler.stopTracking");
    await writeFile(resolve(out, `${label}-webkit-timeline.json`), JSON.stringify({ timeline, scripts, cpu, cpuError }));
    session.removeListener("Timeline.eventRecorded", timelineListener);
    session.removeListener("ScriptProfiler.trackingComplete", scriptListener);
    session.removeListener("CPUProfiler.trackingUpdate", cpuListener);
    if (heap) {
      await session.send("Heap.enable");
      const snapshot = await session.send("Heap.snapshot");
      await writeFile(resolve(out, `${label}-webkit.heapsnapshot`), String(snapshot.snapshotData));
    }
  };
}
