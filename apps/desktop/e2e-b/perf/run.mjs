import { spawn } from "node:child_process";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
import { rendererRss } from "../../e2e/perf/rss.mjs";
const binary = resolve(process.argv[2] ?? "target/debug/openspider-desktop");
const importFixture = resolve(
  process.env.OS_PERF_IMPORT_FIXTURE ?? "target/ui-perf-import.ospider",
);
const fixture = resolve(
  process.env.OS_PERF_FIXTURE ?? "target/ui-perf.ospider",
);
if (!["win32", "darwin"].includes(process.platform))
  throw new Error("Native performance run requires macOS/Windows GUI");
const home = await mkdtemp(join(tmpdir(), "os-perf-native-"));
const port = await new Promise((resolvePort) => {
  const s = createServer();
  s.listen(0, "127.0.0.1", () => {
    const n = s.address().port;
    s.close(() => resolvePort(n));
  });
});
const cdpPort = await new Promise((resolvePort) => {
  const s = createServer();
  s.listen(0, "127.0.0.1", () => {
    const n = s.address().port;
    s.close(() => resolvePort(n));
  });
});
const started = performance.now();
const child = spawn(binary, [], {
  env: {
    ...process.env,
    OPENSPIDER_HOME: home,
    OPENSPIDER_SECRET_STORE: "memory",
    TAURI_WEBDRIVER_PORT: String(port),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`,
  },
  stdio: ["ignore", "pipe", "pipe"],
});
let tracing = false;
let log = "",
  session,
  cdp;
child.stdout.on("data", (d) => (log += d));
child.stderr.on("data", (d) => (log += d));
const metrics = {};
const details = {
  samples: [],
  trace: process.platform === "win32" ? "CDP trace requested" : "WKWebView",
  errors: [],
};
const metric = (value, budget, direction = "max") => ({
  value,
  budget,
  direction,
  passed:
    Number.isFinite(value) &&
    (direction === "max" ? value <= budget : value >= budget),
});
const request = async (path, body, method = "POST") => {
  const r = await fetch(`http://127.0.0.1:${port}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    signal: AbortSignal.timeout(30000),
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const v = await r.json();
  if (!r.ok || v.value?.error) throw new Error(JSON.stringify(v));
  return v.value;
};
const execute = (script) =>
  request(`/session/${session}/execute/sync`, { script, args: [] });
async function until(fn, ms = 30000) {
  const deadline = performance.now() + ms;
  while (performance.now() < deadline) {
    const r = await fn();
    if (r) return r;
    await delay(50);
  }
  throw new Error("Native readiness timeout");
}
async function asyncExecute(expression) {
  await execute(
    `window.__perfResult=null; Promise.resolve(${expression}).then(value=>window.__perfResult={value},e=>window.__perfResult={error:String(e)});return true;`,
  );
  const r = await until(() => execute("return window.__perfResult"), 120000);
  if (r.error) throw new Error(r.error);
  return r.value;
}
async function finishTrace() {
  if (cdp && tracing) {
    try {
      const complete = new Promise((r) =>
        cdp.once("Tracing.tracingComplete", r),
      );
      await cdp.send("Tracing.end");
      tracing = false;
      const { stream } = await Promise.race([
        complete,
        delay(10000).then(() => {
          throw new Error("CDP trace completion timeout");
        }),
      ]);
      let trace = "";
      while (true) {
        const r = await cdp.send("IO.read", { handle: stream });
        trace += r.base64Encoded
          ? Buffer.from(r.data, "base64").toString("utf8")
          : r.data;
        if (r.eof) break;
      }
      await cdp.send("IO.close", { handle: stream });
      await writeFile("target/loadtest/webview2-trace.json", trace);
    } catch (e) {
      details.errors.push(String(e));
    }
  }
}
try {
  child.on("error", (e) => details.errors.push(String(e)));
  await until(async () => {
    try {
      session = (
        await request("/session", { capabilities: { alwaysMatch: {} } })
      ).sessionId;
      return session;
    } catch {
      if (child.exitCode !== null) throw new Error(`App exited: ${log}`);
      return false;
    }
  });
  await until(() =>
    execute(
      'return document.querySelector("[data-shell-ready]")?.getAttribute("data-shell-ready")==="true"',
    ),
  );
  details.userAgent = await execute("return navigator.userAgent");
  metrics.coldStartMs = metric(
    performance.now() - started,
    process.platform === "darwin" ? 2000 : 3000,
  );
  if (process.platform === "win32") {
    const { chromium } = await import("playwright");
    const browser = await chromium.connectOverCDP(
      `http://127.0.0.1:${cdpPort}`,
    );
    cdp = await browser
      .contexts()[0]
      .newCDPSession(browser.contexts()[0].pages()[0]);
    details.browserVersion = await cdp.send("Browser.getVersion");
  }
  const openStart = performance.now();
  await asyncExecute(`window.perfShell.open(${JSON.stringify(fixture)})`);
  await until(
    () =>
      execute(
        `return [...document.querySelectorAll('[role="grid"]')].some(e=>e.textContent.includes("ui.test"))`,
      ),
    120000,
  );
  metrics.openMs = metric(performance.now() - openStart, 3000);
  const overviewStart = performance.now();
  await asyncExecute("window.perfShell.completeSidebars()");
  metrics.overviewIssuesMs = metric(performance.now() - openStart, 5000);
  details.overviewRequestMs = performance.now() - overviewStart;
  const importStart = performance.now();
  details.import = await asyncExecute(
    `window.perfShell.importFixture(${JSON.stringify(importFixture)})`,
  );
  metrics.importMillionMs = metric(performance.now() - importStart, 60000);
  if (cdp) {
    await cdp.send("Tracing.start", {
      categories: "devtools.timeline,blink,cc",
      transferMode: "ReturnAsStream",
    });
    tracing = true;
  }
  details.frames = [];
  for (const axis of ["vertical", "horizontal"]) {
    const frames = await asyncExecute(
      `window.perfShell.scrollFrames('${axis}')`,
    );
    details.frames.push({ axis, ...frames });
    metrics[`${axis}Fps`] = metric(frames.fps, 55, "min");
    metrics[`${axis}MaxFrameMs`] = metric(frames.maxFrameMs, 50);
  }
  await finishTrace();
  // spec: UI-147 [P1]
  const seconds =
    process.platform === "win32"
      ? Number(process.env.OS_PERF_SOAK_SECONDS ?? 1800)
      : 0;
  details.configuredSeconds = seconds;
  if (seconds) {
    for (let i = 0; i < 30; i++)
      await execute(
        `document.querySelectorAll('.os-grid-scroll').forEach(e=>e.scrollTop=${i * 1337});return true;`,
      );
    await delay(2000);
    let seed = 19;
    const start = performance.now();
    details.samples.push({ at: 0, ...rendererRss(child.pid) });
    while (performance.now() - start < seconds * 1000) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      await execute(
        `document.querySelectorAll('.os-grid-scroll').forEach(e=>e.scrollTop=${seed % 100000000});return true;`,
      );
      await delay(1000);
      if (performance.now() - start - details.samples.at(-1).at > 10000)
        details.samples.push({
          at: performance.now() - start,
          ...rendererRss(child.pid),
        });
    }
    details.samples.push({
      at: performance.now() - start,
      ...rendererRss(child.pid),
    });
    metrics.peakRssBytes = metric(
      Math.max(...details.samples.map((s) => s.bytes)),
      400 * 1024 * 1024,
    );
    metrics.growthPercent = metric(
      (details.samples.at(-1).bytes / details.samples[0].bytes - 1) * 100,
      10,
    );
    details.fullSoak = seconds >= 1800;
  } else
    details.memory = "macOS uses attributable Playwright WebKit fallback (7.4)";
} catch (error) {
  details.errors.push(String(error));
  process.exitCode = 1;
} finally {
  await finishTrace();
  const report = {
    scenario: "native",
    size: process.env.OS_PERF_ROWS === "5000000" ? "large" : "small",
    started_at: new Date().toISOString(),
    engine: process.platform === "darwin" ? "wkwebview" : "webview2",
    metrics,
    details,
    passed:
      !details.errors.length && Object.values(metrics).every((m) => m.passed),
  };
  await mkdir("target/loadtest/raw", { recursive: true });
  await writeFile(
    "target/loadtest/raw/native.json",
    JSON.stringify(report, null, 2),
  );
  await writeFile("target/loadtest/native.log", log);
  if (session)
    await request(`/session/${session}`, undefined, "DELETE").catch(() => {});
  if (child.exitCode === null) {
    child.kill();
    await Promise.race([
      new Promise((r) => child.once("exit", r)),
      delay(5000),
    ]);
    if (child.exitCode === null) child.kill("SIGKILL");
  }
  await rm(home, {
    recursive: true,
    force: true,
    maxRetries: 10,
    retryDelay: 100,
  });
  if (!report.passed) process.exitCode = 1;
}
