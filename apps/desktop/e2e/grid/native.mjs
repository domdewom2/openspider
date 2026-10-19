// Real WKWebView / WebView2 through the app's optional W3C WebDriver plugin.
// Launches only the supplied, locally built spike binary; all traffic stays on loopback.
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
const binary = process.argv[2];
if (!binary) throw new Error("Usage: node e2e/grid/native.mjs <spike binary> [report directory]");
const output = resolve(process.argv[3] ?? "test-results/grid-native");
await mkdir(output, { recursive: true });
const home = await mkdtemp(join(tmpdir(), "os-grid-native-"));
const port = await new Promise((resolvePort, reject) => {
  const server = createServer(); server.on("error", reject);
  server.listen(0, "127.0.0.1", () => { const port = server.address().port; server.close(() => resolvePort(port)); });
});
const child = spawn(resolve(binary), [], { env: { ...process.env, OPENSPIDER_HOME: home, OPENSPIDER_STUB_VIEWS: "1", OPENSPIDER_SECRET_STORE: "memory", TAURI_WEBDRIVER_PORT: String(port), ...(process.platform === "win32" ? { WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--force-device-scale-factor=${process.env.OS_GRID_SCALE ?? "1"}` } : {}) }, stdio: ["ignore", "pipe", "pipe"] });
let log = "";
child.stdout.on("data", d => { log += d; }); child.stderr.on("data", d => { log += d; });
let session;
const report = { platform: process.platform, native: true, processScale: Number(process.env.OS_GRID_SCALE ?? 1), zoom: [], frames: [], errors: [] };
async function request(path, body, method = "POST") {
  const response = await fetch(`http://127.0.0.1:${port}${path}`, { method, headers: { "Content-Type": "application/json" }, signal: AbortSignal.timeout(40_000), ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw new Error(JSON.stringify(result));
  return result.value;
}
const execute = script => request(`/session/${session}/execute/sync`, { script, args: [] });
async function asyncExecute(expression) {
  // A pending execute/async can hold the native evaluation queue needed by Tauri IPC.
  // Start the promise with a synchronous evaluation, then poll its serializable result.
  await execute(`window.__gridResult = null; Promise.resolve(${expression}).then(value => { window.__gridResult = {value}; }, error => { window.__gridResult = {value: {error:String(error)}}; }); return true;`);
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const result = await execute("return window.__gridResult");
    if (result !== null) return result.value;
    await delay(100);
  }
  throw new Error(`Measurement timed out: ${expression}`);
}

try {
  for (let i = 0; i < 100; i++) {
    try { const result = await request("/session", { capabilities: { alwaysMatch: {} } }); session = result.sessionId; break; }
    catch { if (child.exitCode !== null) throw new Error(`App exited ${child.exitCode}: ${log}`); await delay(200); }
  }
  if (!session) throw new Error("Native WebDriver unavailable");
  await request(`/session/${session}/timeouts`, { script: 30_000 });
  for (let i = 0; i < 100; i++) {
    if (await execute("return !!window.gridSpike?.ready()")) break;
    await delay(200);
  }
  if (!await execute("return !!window.gridSpike?.ready()")) throw new Error("Native grid did not become ready");
  report.activation = await asyncExecute("window.gridSpike.activate()");
  await delay(300);
  report.initial = await execute("return window.gridSpike.snapshot()");
  if (report.initial.visibility !== "visible") {
    // A hidden WKWebView may still load data, but rAF cannot establish paint latency or FPS.
    await execute("window.gridSpike.jump(4999999); return true");
    for (let i = 0; i < 50; i++) {
      report.unpaintedJump = await execute("return window.gridSpike.snapshot()");
      if (report.unpaintedJump.last === 5_000_000) break;
      await delay(100);
    }
    throw new Error("Native window remains hidden after show/setFocus; paint, FPS and zoom/hit-test gates require an active desktop session");
  }
  report.jump = await asyncExecute("window.gridSpike.jump(4999999)");
  for (const axis of ["vertical", "horizontal"]) report.frames.push(await asyncExecute(`window.gridSpike.record('${axis}')`));
  await asyncExecute("window.gridSpike.jump(400)");
  for (const zoom of [0.8, 1, 1.25, 1.5, 2]) {
    const before = await execute("return window.gridSpike.snapshot()");
    const after = await asyncExecute(`window.gridSpike.zoom(${zoom})`);
    if (after.error) throw new Error(after.error);
    const row = after.visible.y + 2;
    const bounds = await execute(`return window.gridSpike.bounds(1, ${row})`);
    await request(`/session/${session}/actions`, { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [
      { type: "pointerMove", duration: 0, x: Math.round(bounds.x + bounds.width / 2), y: Math.round(bounds.y + bounds.height / 2), origin: "viewport" },
      { type: "pointerDown", button: 0 }, { type: "pointerUp", button: 0 },
    ] }] });
    await delay(150);
    const clicked = await execute("return window.gridSpike.snapshot()");
    report.zoom.push({ zoom, before, after, clicked, displayCellWidth: after.cellWidth * report.initial.innerWidth / after.innerWidth, checks: {
      backingStore: after.canvas.every(c => Math.abs(c.widthError) <= 1 && Math.abs(c.heightError) <= 1),
      cellWidth: Math.abs(after.cellWidth * report.initial.innerWidth / after.innerWidth - 320 * zoom) <= 1,
      anchor: before.visible.y === after.visible.y,
      header: after.headerBounds ? after.bounds.y + 1 >= after.headerBounds.y + after.headerBounds.height : null,
      selection: JSON.stringify(before.selection) === JSON.stringify(after.selection), hit: clicked.focus === row + 1,
    } });
    const screenshot = await request(`/session/${session}/screenshot`, undefined, "GET");
    await writeFile(join(output, `zoom-${zoom}.png`), Buffer.from(screenshot, "base64"));
  }
  report.errors = await execute("return window.gridSpike.errors");
} catch (error) { report.errors.push(String(error)); report.failureSnapshot = session ? await execute("return window.gridSpike?.snapshot()").catch(() => null) : null; process.exitCode = 1; }
finally {
  await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2));
  await writeFile(join(output, "native.log"), log);
  if (session) await request(`/session/${session}`, undefined, "DELETE").catch(() => {});
  if (child.exitCode === null) { const exited = new Promise(resolveExit => child.once("exit", resolveExit)); child.kill(); await Promise.race([exited, delay(5000).then(() => child.kill("SIGKILL"))]); }
  await rm(home, { recursive: true, force: true });
}
