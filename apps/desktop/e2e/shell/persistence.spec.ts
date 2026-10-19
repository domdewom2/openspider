import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { once } from "node:events";
import { test, expect, type Page } from "@playwright/test";
import { bridgeBinary } from "../support/bridge";
import fixture from "../../src/features/shell/__fixtures__/ui-state-v1.json" with { type: "json" };

/** Unlike a page reload, this keeps the home directory and restarts the actual Rust process. */
async function start(home: string) {
  const child = spawn(bridgeBinary(), ["--home", home], { stdio: ["ignore", "pipe", "inherit"], env: { ...process.env, OPENSPIDER_SECRET_STORE: "memory" } });
  try {
    const info = await new Promise<{ port: number; token: string }>((resolve, reject) => {
      let output = "";
      const timer = setTimeout(() => reject(new Error("Bridge startup timeout")), 20_000);
      child.once("error", (error) => { clearTimeout(timer); reject(error); });
      child.once("exit", () => { clearTimeout(timer); reject(new Error("Bridge exited before startup")); });
      child.stdout.on("data", (chunk: Buffer) => {
        output += chunk.toString();
        if (output.includes("\n")) {
          clearTimeout(timer);
          try { resolve(JSON.parse(output.split("\n")[0]!)); } catch (e) { reject(e); }
        }
      });
    });
    return { url: `ws://127.0.0.1:${info.port}/?token=${info.token}`, async stop() {
      const exited = once(child, "exit"); child.kill(); await exited;
    } };
  } catch (e) { child.kill(); throw e; }
}
async function open(page: Page, url: string) {
  await page.goto(`/?bridge=${encodeURIComponent(url)}`);
  await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
}

// spec: UI-125, UI-021 [P1]
test("migrated fixture and global column reset survive an actual core and frontend restart", async ({ page }) => {
  const home = await mkdtemp(join(tmpdir(), "os-shell-restart-"));
  await writeFile(join(home, "ui-state.json"), JSON.stringify(fixture));
  let bridge: Awaited<ReturnType<typeof start>> | undefined;
  try {
    bridge = await start(home);
    await page.setViewportSize({ width: 1440, height: 900 });
    await open(page, bridge.url);
    await expect(page.locator("#shell-detail")).toBeHidden();
    expect((await page.locator(".os-app-sidebar").boundingBox())!.width).toBeCloseTo(500, 0);
    await page.evaluate(async () => {
      const path = "/src/ui/actions.ts";
      const { runAction } = await import(/* @vite-ignore */ path);
      runAction("view.resetColumns");
    });
    await page.getByRole("button", { name: "Reset", exact: true }).click();
    await expect.poll(async () => JSON.parse(await readFile(join(home, "ui-state.json"), "utf8")).state.tables).toBeUndefined();
    const saved = JSON.parse(await readFile(join(home, "ui-state.json"), "utf8"));
    expect(saved.state.columnTemplates).toEqual(fixture.state.columnTemplates);
    expect(saved.state.tabs).toEqual(fixture.state.tabs);
    await page.goto("about:blank");
    await bridge.stop(); bridge = await start(home);
    await open(page, bridge.url);
    await expect(page.locator("#shell-detail")).toBeHidden();
    expect((await page.locator(".os-app-sidebar").boundingBox())!.width).toBeCloseTo(500, 0);
    const loaded = await page.evaluate(async () => {
      const path = "/src/state/layout.ts";
      return (await import(/* @vite-ignore */ path)).useLayout.getState().sections;
    });
    expect(loaded).toEqual(saved.state);
  } finally { await page.goto("about:blank"); await bridge?.stop(); await rm(home, { recursive: true, force: true }); }
});

// spec: UI-125 [P1]
test("malformed JSON is backed up and defaults load without a crash", async ({ page }) => {
  const home = await mkdtemp(join(tmpdir(), "os-shell-corrupt-"));
  const corrupt = '{"version":1, broken json';
  await writeFile(join(home, "ui-state.json"), corrupt);
  let bridge: Awaited<ReturnType<typeof start>> | undefined;
  try {
    bridge = await start(home);
    await page.setViewportSize({ width: 1440, height: 900 });
    await open(page, bridge.url);
    expect((await page.locator(".os-app-sidebar").boundingBox())!.width).toBeCloseTo(360, 0);
    await expect.poll(() => readFile(join(home, "ui-state.json.bak"), "utf8").catch(() => "")).toBe(corrupt);
    const saved = JSON.parse(await readFile(join(home, "ui-state.json"), "utf8"));
    expect(saved.version).toBe(2);
    expect(saved.state.layout.sidebar.share).toBe(.25);
  } finally { await page.goto("about:blank"); await bridge?.stop(); await rm(home, { recursive: true, force: true }); }
});
