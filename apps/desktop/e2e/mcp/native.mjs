// spec: AGENT-002
// macOS E2E-B: production AccessPanel/adapter, 1.5 s delayed token response, actual system clipboard.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { mkdtemp, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir, release } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
if (process.platform !== "darwin") throw new Error("macOS production clipboard check requires WKWebView");
const binary = process.argv[2];
if (!binary) throw new Error("Usage: node e2e/mcp/native.mjs <e2e-webdriver binary> [evidence directory]");
const output = resolve(process.argv[3] ?? "test-results/mcp-native");
await mkdir(output, { recursive: true });
const home = await mkdtemp(join(tmpdir(), "u25-native-"));
const original = execFileSync("/usr/bin/pbpaste", { encoding: "utf8" });
const port = await new Promise(resolvePort => {
  const server = createServer(); server.listen(0, "127.0.0.1", () => { const port = server.address().port; server.close(() => resolvePort(port)); });
});
const child = spawn(resolve(binary), [], { env: { ...process.env, OPENSPIDER_HOME: home, OPENSPIDER_SECRET_STORE: "memory", TAURI_WEBDRIVER_PORT: String(port) }, stdio: ["ignore", "pipe", "pipe"] });
let log = "";
child.stdout.on("data", chunk => { log += chunk; }); child.stderr.on("data", chunk => { log += chunk; });
let session;
const report = { timestamp: new Date().toISOString(), platform: process.platform, arch: process.arch, kernel: release(), engine: "WKWebView", delayMs: 1500, nativeClipboard: false, domRedacted: false, restored: false, errors: [] };
async function request(path, body, method = "POST") {
  const response = await fetch(`http://127.0.0.1:${port}${path}`, { method, headers: { "Content-Type": "application/json" }, signal: AbortSignal.timeout(20_000), ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw new Error(JSON.stringify(result));
  return result.value;
}
const execute = script => request(`/session/${session}/execute/sync`, { script, args: [] });
try {
  for (let i = 0; i < 100; i++) {
    try { session = (await request("/session", { capabilities: { alwaysMatch: {} } })).sessionId; break; }
    catch { if (child.exitCode !== null) throw new Error(`App exited: ${log}`); await delay(200); }
  }
  assert.ok(session, "Native WebDriver must start");
  for (let i = 0; i < 100; i++) { if (await execute("return !!window.mcpNative && !!document.querySelector('button')")) break; await delay(100); }
  const result = await request(`/session/${session}/element`, { using: "xpath", value: "//button[normalize-space(.)='Copy token']" });
  await request(`/session/${session}/element/${result['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
  for (let i = 0; i < 100; i++) {
    const copied = execFileSync("/usr/bin/pbpaste", { encoding: "utf8" });
    if (copied === "u25-native-delayed-token") { report.nativeClipboard = true; break; }
    await delay(100);
  }
  assert.ok(report.nativeClipboard, "Delayed token must reach the actual macOS clipboard");
  report.domRedacted = await execute("return !document.body.innerHTML.includes('u25-native-delayed-token')");
  assert.ok(report.domRedacted, "Copy must keep token out of DOM");
  assert.ok(await execute("return document.body.textContent.includes('Copied to clipboard.')"));
} catch (error) { report.errors.push(String(error)); process.exitCode = 1; }
finally {
  if (session) {
    await execute(`window.__restored = false; window.mcpNative.restore(${JSON.stringify(original)}).then(() => { window.__restored = true; }); return true;`).catch(() => {});
    for (let i = 0; i < 20; i++) { if (await execute("return window.__restored").catch(() => false)) break; await delay(100); }
    report.restored = execFileSync("/usr/bin/pbpaste", { encoding: "utf8" }) === original;
    if (!report.restored) { report.errors.push("Original text clipboard was not restored"); process.exitCode = 1; }
    await request(`/session/${session}`, undefined, "DELETE").catch(() => {});
  }
  if (child.exitCode === null) { const exited = new Promise(resolveExit => child.once("exit", resolveExit)); child.kill(); await Promise.race([exited, delay(5_000).then(() => child.kill("SIGKILL"))]); }
  await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2));
  await writeFile(join(output, "native.log"), log);
  await rm(home, { recursive: true, force: true });
}
