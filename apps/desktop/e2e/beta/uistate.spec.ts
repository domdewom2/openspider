import { test, expect } from "@playwright/test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { persistentBridge } from "./persistent-bridge";

// spec: UI-125
// spec: UI-124
test("overlapping detail and recent patches survive a real process restart", async ({ page }) => {
  const home = mkdtempSync(join(tmpdir(), "os-n03-uistate-"));
  let bridge = await persistentBridge(home);
  try {
    const open = async () => {
      await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
      await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    };
    await open();
    const revisions = await page.evaluate(async () => {
      const { uistate } = await import(/* @vite-ignore */ String("/src/ipc/uistate.ts"));
      const { updateDetailState } = await import(/* @vite-ignore */ String("/src/features/detail/frame/persistence.ts"));
      const { updateRecent } = await import(/* @vite-ignore */ String("/src/features/help/recent.ts"));
      const { flushUiState } = await import(/* @vite-ignore */ String("/src/state/uistate.ts"));
      await flushUiState();
      const load = uistate.load;
      const seen: number[] = [];
      let release!: () => void;
      const overlap = new Promise<void>(resolve => { release = resolve; });
      // Both feature writers must read the same on-disk revision before either saves.
      uistate.load = async () => {
        const dto = await load();
        if (seen.length < 2) {
          seen.push(dto.revision);
          if (seen.length === 2) release();
          await overlap;
        }
        return dto;
      };
      try {
        await Promise.all([
          updateDetailState((state: Record<string, unknown>) => ({ ...state, detail: { hidden: ["cookies"] } })),
          updateRecent((entries: { path: string }[]) => [{ path: "/fixture/concurrent.ospider" }, ...entries]),
        ]);
      } finally { uistate.load = load; }
      return seen;
    });
    expect(revisions).toHaveLength(2);
    expect(revisions[0]).toBe(revisions[1]);
    await page.goto("about:blank");
    await bridge.stop();
    bridge = await persistentBridge(home);
    await open();
    const restored = await page.evaluate(async () => {
      const { uistate } = await import(/* @vite-ignore */ String("/src/ipc/uistate.ts"));
      return (await uistate.load()).state;
    });
    expect(restored).toMatchObject({ detail: { hidden: ["cookies"] }, recentCrawls: [{ path: "/fixture/concurrent.ospider" }] });
  } finally { await bridge.stop(); rmSync(home, { recursive: true, force: true }); }
});
