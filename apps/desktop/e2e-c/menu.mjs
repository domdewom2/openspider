// @ts-check
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile, writeFile, mkdir, stat } from "node:fs/promises";
import { join } from "node:path";
/** @param {{browser: import('../e2e-b/u32-cases.mjs').NativeBrowser, evaluate: (script: string) => Promise<any>, check: (name: string, fn: () => Promise<any>) => Promise<void>, output: string, appPid: number}} ctx */
export async function menuCases({ browser, evaluate, check, output, appPid }) {
  assert.equal(process.platform, "darwin", "E2E-C requires the real macOS menu bar");
  const helper = process.env.U32_MENU_HELPER;
  assert.ok(helper, "Compile e2e-c/menu.swift and set U32_MENU_HELPER");
  for (const lang of ["de", "en"]) {
    await evaluate(`import('/src/state/prefs.ts').then(m => m.usePrefs.getState().patch({general:{language:'${lang}'}}))`);
    // spec: UI-002
    await check(`U32 E2E-C ${lang}: native AX menu snapshot`, async () => {
      const native = JSON.parse(execFileSync(helper, [String(appPid)], { encoding: "utf8", timeout: 15_000 }));
      const model = await browser.execute("return window.__osMenu.model");
      const nativeTitles = native.children.map((/** @type {{title: string}} */ c) => c.title);
      for (const menu of model.menus) {
        assert.ok(nativeTitles.includes(menu.label), `Native menu missing: ${menu.label}`);
        const capture = join(output, `menu-${lang}-${menu.id}.png`);
        execFileSync(helper, [String(appPid), menu.label, capture], { timeout: 15_000 });
        assert.ok((await stat(capture)).size > 1000, `Native menu screenshot missing: ${menu.label}`);
      }
      const snapshot = JSON.stringify(native, null, 2) + "\n";
      await writeFile(join(output, `menu-${lang}.json`), snapshot);
      const baselines = new URL("./baselines/darwin/", import.meta.url);
      if (process.env.U32_UPDATE_NATIVE === "1") { await mkdir(baselines, { recursive: true }); await writeFile(new URL(`menu-${lang}.json`, baselines), snapshot); }
      else assert.equal(snapshot, await readFile(new URL(`menu-${lang}.json`, baselines), "utf8"));
      return { nativeMenus: nativeTitles, language: lang };
    });
    // spec: UI-002
    // spec: UI-113 [P1]
    await check(`U32 E2E-C ${lang}: native Help opens shortcut dialog`, async () => {
      const title = lang === "de" ? "Tastaturkürzel" : "Keyboard shortcuts";
      execFileSync(helper, [String(appPid), title], { timeout: 15_000 });
      await browser.waitUntil(async () => browser.execute("return !!document.querySelector('[role=dialog]')"));
      assert.equal(await browser.execute("return !!document.querySelector('[role=dialog] [data-shortcut]')"), true);
      await browser.saveScreenshot(join(output, `help-${lang}.png`));
      await browser.execute("document.querySelector('[role=dialog] button').click(); return true;");
    });
  }
}
