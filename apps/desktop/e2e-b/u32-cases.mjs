// @ts-check
// Native-only cases. No browser mocks: the runner supplies its real WebDriver/IPC session.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

/** @typedef {{execute: (script: string) => Promise<any>, getWindowHandles: () => Promise<string[]>, switchToWindow: (id: string) => Promise<unknown>, closeWindow: () => Promise<unknown>, waitUntil: (fn: () => Promise<any>) => Promise<unknown>, saveScreenshot: (path: string) => Promise<unknown>}} NativeBrowser */
/** @param {{browser: NativeBrowser, evaluate: (script: string) => Promise<any>, check: (name: string, fn: () => Promise<any>) => Promise<void>, until: (fn: () => Promise<any>, label: string) => Promise<any>, output: string, binary: string, startFixture: () => Promise<any>, appPid?: number}} ctx */
export async function nativeCases({ browser, evaluate, check, until, output, binary, startFixture, appPid }) {
  // spec: UI-113 [P1]
  await check("U32 native clipboard: production write and OS read", async () => {
    const original = process.platform === "darwin" ? execFileSync("/usr/bin/pbpaste", { encoding: "utf8" }) : execFileSync("powershell.exe", ["-NoProfile", "-Command", "Get-Clipboard -Raw"], { encoding: "utf8" }).replace(/\r?\n$/, "");
    try {
      await evaluate("import('/src/harness/native.ts').then(m => m.copyDesktopText('u32-native-clipboard'))");
      const actual = process.platform === "darwin" ? execFileSync("/usr/bin/pbpaste", { encoding: "utf8" }) : execFileSync("powershell.exe", ["-NoProfile", "-Command", "Get-Clipboard -Raw"], { encoding: "utf8" }).trimEnd();
      assert.equal(actual, "u32-native-clipboard");
    } finally { await evaluate(`import('/src/harness/native.ts').then(m => m.copyDesktopText(${JSON.stringify(original)}))`); }
  });
  // spec: UI-001
  await check("U32 multiple windows: singleton settings and independent close", async () => {
    const before = await browser.getWindowHandles();
    await evaluate("window.s02.settings()");
    const handles = await until(async () => { const h = await browser.getWindowHandles(); return h.length === before.length + 1 && h; }, "settings window");
    await evaluate("window.s02.settings()");
    assert.equal((await browser.getWindowHandles()).length, handles.length);
    const settings = handles.find((/** @type {string} */ h) => !before.includes(h));
    assert.ok(settings);
    await browser.switchToWindow(settings);
    await until(() => browser.execute("return window.__osRuntime?.label === 'settings' && document.body.innerText.length > 0"), "settings content");
    await browser.closeWindow();
    await browser.switchToWindow(before[0]);
    assert.equal(await browser.execute("return window.__osRuntime?.label"), "main");
  });
  for (const kind of ["open", "save"]) {
    // spec: UI-002
    await check(`U32 native ${kind} dialog: OS visibility and cancellation`, async () => {
      await browser.execute(`window.__u32Dialog = { pending: true }; import('/src/ipc/client.ts').then(m => m.client.invoke('dialog.${kind}', {options:{title:'OpenSpider U32 ${kind}',multiple:false}})).then(value => {window.__u32Dialog={value};}, error => {window.__u32Dialog={error:String(error)};}); return true;`);
      const helper = fileURLToPath(new URL("./u32-dialog.mjs", import.meta.url));
      const native = JSON.parse(execFileSync(process.execPath, [helper, binary, `OpenSpider U32 ${kind}`, "cancel", String(appPid ?? "")], { encoding: "utf8", timeout: 30_000 }));
      assert.equal(native.visible, true, "OS must observe a native file dialog");
      const result = await until(async () => { const r = await browser.execute("return window.__u32Dialog"); return !r.pending && r; }, "native dialog cancellation");
      assert.deepEqual(result, { value: null });
      return native;
    });
  }
  for (const kind of ["open", "save"]) {
    // spec: UI-002
    await check(`U32 native ${kind} dialog: positive path selection`, async () => {
      const folder = await mkdtemp(join(tmpdir(), "u32-dialog-"));
      const path = join(folder, kind === "open" ? "existing.txt" : "new.txt");
      if (kind === "open") await writeFile(path, "U32 fixture\n");
      try {
        await browser.execute(`window.__u32Dialog={pending:true}; import('/src/ipc/client.ts').then(m=>m.client.invoke('dialog.${kind}',{options:{title:'OpenSpider U32 ${kind}',defaultPath:${JSON.stringify(path)},multiple:false}})).then(value=>{window.__u32Dialog={value};},error=>{window.__u32Dialog={error:String(error)};}); return true;`);
        const helper = fileURLToPath(new URL("./u32-dialog.mjs", import.meta.url));
        const native = JSON.parse(execFileSync(process.execPath, [helper, binary, `OpenSpider U32 ${kind}`, "accept", String(appPid ?? "")], {encoding:"utf8",timeout:30_000}));
        assert.equal(native.visible, true);
        const result = await until(async () => { const r=await browser.execute("return window.__u32Dialog");return !r.pending && r; }, "positive native dialog result");
        assert.equal(normalize(result.value), normalize(path));
        return { kind, selectedFixture: true };
      } finally { await rm(folder,{recursive:true,force:true}); }
    });
  }
  // spec: UI-016
  await check("U32 native notification: real permission and plugin delivery", async () => {
    const granted = await evaluate("import('/src/harness/native.ts').then(m => m.notificationPermission())");
    assert.equal(granted, true, "Orchestrator must grant OS notification permission before this test");
    await evaluate("import('/src/harness/native.ts').then(m => m.sendDesktopNotification('OpenSpider U32', 'Native notification fixture'))");
    await browser.saveScreenshot(`${output}/notification-requested.png`);
    return { permission: granted, pluginAccepted: true, osVisualConfirmation: "required in acceptance log" };
  });
  // spec: UI-001
  await check("U32 native HiDPI: actual window factor and canvas backing store", async () => {
    await startFixture();
    await browser.execute("document.querySelector('.u34-nav [data-group=urls] .u34-nav-button').click(); return true;");
    await until(() => browser.execute("return [...document.querySelectorAll('canvas')].some(c=>c.clientWidth>0)"), "table canvas");
    const result = await evaluate("import('/src/harness/native.ts').then(m => m.dpi())");
    const expected = Number(process.env.U32_EXPECT_DSF);
    assert.ok([1, 1.25, 1.5, 2].includes(expected), "Set U32_EXPECT_DSF to the actual OS scale; rerun at each required scale");
    assert.ok(Math.abs(result.native - expected) < 0.01);
    assert.ok(Math.abs(result.dom - expected) < 0.01);
    assert.ok(result.canvases.length > 0, "Select a table with canvas before checking HiDPI");
    for (const canvas of result.canvases) assert.ok(Math.abs(canvas.backing - canvas.css * expected) <= 1);
    return result;
  });
}
