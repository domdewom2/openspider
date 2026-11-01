import { test } from "@playwright/test";
import { spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { toolBinary } from "../alpha/support";
import { bridgeBinary } from "../support/bridge";

// M2-N25 Diagnose (kein Teil der Suite; zum Ausführen nach apps/desktop/e2e/n25/ kopieren): Rate vor Pause / nach Fortsetzen in der App, CPU je Prozess und Thread,
// Overview-Werte über die Zeit. Nur mit OS_N25_PROBE=1; Ergebnis als JSON in OS_N25_OUT (Standard: tmp).
// Linux-only (/proc). Parameter: OS_N25_PAGES, OS_N25_DELAY, OS_N25_PAUSE_AT, OS_N25_UI (=0: Seite nach dem Start leer).
const PAGES = Number(process.env.OS_N25_PAGES ?? 6000);
const DELAY = Number(process.env.OS_N25_DELAY ?? 50);
const LINKS = Number(process.env.OS_N25_LINKS ?? 30);
const PAUSE_AT = Number(process.env.OS_N25_PAUSE_AT ?? 3000);
const UI = process.env.OS_N25_UI !== "0";
const TICK = 100;

function procCpu(pid: number): number | null {
  try {
    const raw = readFileSync(`/proc/${pid}/stat`, "utf8");
    const parts = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
    return (Number(parts[11]) + Number(parts[12])) / TICK;
  } catch { return null; }
}
function threadCpu(pid: number): Record<string, number> {
  const out: Record<string, number> = {};
  try {
    for (const t of readdirSync(`/proc/${pid}/task`)) {
      try {
        const raw = readFileSync(`/proc/${pid}/task/${t}/stat`, "utf8");
        const name = raw.slice(raw.indexOf("(") + 1, raw.lastIndexOf(")"));
        const parts = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
        out[name] = (out[name] ?? 0) + (Number(parts[11]) + Number(parts[12])) / TICK;
      } catch { /* thread ended */ }
    }
  } catch { /* process ended */ }
  return out;
}
function walBytes(home: string): number {
  try {
    const dir = join(home, "crawls");
    return readdirSync(dir).filter(f => f.endsWith(".ospider-wal")).reduce((n, f) => n + statSync(join(dir, f)).size, 0);
  } catch { return -1; }
}
/** CPU seconds of all browser processes (chrome/headless_shell/WebKit) and of node (vite). */
function groupCpu(): Record<string, number> {
  const out: Record<string, number> = { browser: 0, node: 0 };
  for (const p of readdirSync("/proc")) {
    if (!/^\d+$/.test(p)) continue;
    try {
      const raw = readFileSync(`/proc/${p}/stat`, "utf8");
      const name = raw.slice(raw.indexOf("(") + 1, raw.lastIndexOf(")"));
      const parts = raw.slice(raw.lastIndexOf(")") + 2).split(" ");
      const cpu = (Number(parts[11]) + Number(parts[12])) / TICK;
      if (/chrome|headless_shell|WebKit|MiniBrowser/i.test(name)) out.browser! += cpu;
      else if (name === "node" || name === "esbuild") out.node! += cpu;
    } catch { /* gone */ }
  }
  return out;
}

test.skip(!process.env.OS_N25_PROBE, "M2-N25 Diagnose nur mit OS_N25_PROBE=1");

test("n25 resume probe", async ({ page }) => {
  test.setTimeout(900_000);
  page.setDefaultTimeout(20_000);
  const home = mkdtempSync(join(tmpdir(), "os-n25-"));
  const site = spawn(toolBinary("os-testsite"), [`synthetic:n=${PAGES},links=${LINKS},seed=7,delay_ms=${DELAY}`], { stdio: ["ignore", "pipe", "inherit"] });
  const base = await new Promise<string>((resolve, reject) => {
    let output = "";
    site.stdout!.on("data", (d: Buffer) => { output += d.toString(); const m = /-> (http:\/\/127\.0\.0\.1:\d+)/.exec(output); if (m) resolve(m[1]!); });
    site.once("error", reject);
  });
  const bridge: ChildProcess = spawn(bridgeBinary(), ["--home", home], { stdio: ["ignore", "pipe", "inherit"] });
  const info = await new Promise<{ port: number; token: string }>((resolve) => {
    let out = "";
    bridge.stdout!.on("data", (d: Buffer) => { out += d.toString(); const i = out.indexOf("\n"); if (i >= 0) resolve(JSON.parse(out.slice(0, i))); });
  });
  const url = `ws://127.0.0.1:${info.port}/?token=${info.token}`;
  const samples: unknown[] = [];
  const events: unknown[] = [];
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: url })}`);
    await page.locator("[data-shell-ready=true]").waitFor({ state: "attached" });
    await page.evaluate(() => {
      return import(/* @vite-ignore */ String("/src/ipc/client.ts")).then(({ getClient }) => {
        const c = getClient(); const invoke = c.invoke.bind(c);
        const calls: unknown[] = []; Object.assign(globalThis, { __n25Calls: calls });
        c.invoke = async <T>(cmd: string, args?: unknown, signal?: AbortSignal): Promise<T> => {
          const started = performance.now();
          const pending = { cmd, at: Date.now(), pending: true }; calls.push(pending);
          try {
            const result = await invoke(cmd, args, signal) as T;
            const summary = cmd === "sidebar.overview" ? (result as { summary: unknown }).summary : undefined;
            calls.push({ cmd, at: Date.now(), ms: Math.round(performance.now() - started), summary });
            return result;
          } catch (error) { calls.push({ cmd, at: Date.now(), ms: Math.round(performance.now() - started), error: String(error) }); throw error; }
          finally { pending.pending = false; }
        };
      });
    });
    await page.evaluate(async (base) => {
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      const dto = await config.get(); const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 5 } } });
      return crawl.start({ source: { kind: "spider", url: `${base}/` }, scope: "domain" });
    }, base);
    const t0 = Date.now();
    const status = () => page.evaluate(async () => {
      const { useSession } = await import(/* @vite-ignore */ String("/src/state/session.ts"));
      const c = useSession.getState().crawl;
      return c ? { phase: c.phase, crawled: c.crawled, queued: c.queued, rate: c.rate, avg: c.avgRate, threads: c.activeThreads, merging: c.merging } : null;
    });
    const sample = async (label: string) => {
      const s = await status();
      const row = { t: (Date.now() - t0) / 1000, label, ...s, bridgeCpu: procCpu(bridge.pid!), wal: walBytes(home), writer: threadCpu(bridge.pid!)["os-engine-write"] ?? null, ...groupCpu() };
      samples.push(row);
      return s;
    };
    let threadsBefore: Record<string, number> = {};
    for (;;) {
      const s = await sample("run1");
      if ((s?.crawled ?? 0) >= PAUSE_AT) break;
      await page.waitForTimeout(1000);
    }
    threadsBefore = threadCpu(bridge.pid!);
    if (!UI) { /* nothing */ }
    await page.evaluate(async () => { const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts")); await crawl.pause(); });
    events.push({ t: (Date.now() - t0) / 1000, ev: "paused" });
    for (let i = 0; i < 20; i++) { await sample("paused"); await page.waitForTimeout(1000); }
    const threadsPaused = threadCpu(bridge.pid!);
    await page.evaluate(async () => { const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts")); await crawl.resume(); });
    events.push({ t: (Date.now() - t0) / 1000, ev: "resumed" });
    let seenRunning = false;
    for (let i = 0; i < 600; i++) {
      const s = await sample("run2");
      if (s?.phase === "running") seenRunning = true;
      if (seenRunning && s?.phase && s.phase !== "running") break;
      await page.waitForTimeout(1000);
    }
    const threadsEnd = threadCpu(bridge.pid!);
    for (let i = 0; i < 15; i++) { await sample("after"); await page.waitForTimeout(1000); }
    const calls = await page.evaluate(() => (globalThis as unknown as { __n25Calls: unknown[] }).__n25Calls);
    const out = process.env.OS_N25_OUT ?? join(tmpdir(), "n25-probe.json");
    writeFileSync(out, JSON.stringify({ t0, pages: PAGES, delay: DELAY, ui: UI, events, samples, threadsBefore, threadsPaused, threadsEnd, calls }, null, 1));
    console.log("written", out);
  } finally {
    await page.goto("about:blank");
    bridge.kill(); site.kill();
    rmSync(home, { recursive: true, force: true });
  }
});
