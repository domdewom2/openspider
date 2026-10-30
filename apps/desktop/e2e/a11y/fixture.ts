// UI-126: isolated real core, deterministic loopback fixture; no mock fallback.
import { createServer } from "node:http";
import { test as base, expect, type Page, type Locator } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { startBridge } from "../support/bridge";

export { expect };
export const BARS = ["Data tabs", "Detail tabs", "Crawl summary"] as const;
export const test = base.extend<{ audit: { url: string; bridge: string; invoke: <T = unknown>(cmd: string, args?: unknown) => Promise<T>; crawl: () => Promise<void> } }>({
  audit: async ({ page }, use) => {
    const server = createServer((req, res) => {
      res.setHeader("Content-Type", req.url === "/robots.txt" ? "text/plain" : "text/html");
      res.end(req.url === "/robots.txt" ? "User-agent: *\nAllow: /\n" : `<!doctype html><html lang="en"><title>A11y ${req.url}</title><h1>Accessibility fixture</h1><a href="/one">One</a><a href="/two">Two</a></html>`);
    });
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
    const address = server.address();
    if (!address || typeof address === "string") throw new Error("No fixture port");
    const bridge = await startBridge();
    try {
      expect(bridge, "compiled os-app-bridge required").not.toBeNull();
      const url = `http://127.0.0.1:${address.port}/`;
      const invoke = <T,>(cmd: string, args?: unknown) => page.evaluate(async ({ cmd, args }) => {
        const path = "/src/ipc/client.ts";
        const { client } = await import(/* @vite-ignore */ path);
        return client.invoke(cmd, args);
      }, { cmd, args }) as Promise<T>;
      await page.clock.install({ time: new Date("2026-10-30T12:00:00Z") });
      await page.goto(`/?bridge=${encodeURIComponent(bridge!.url)}`);
      await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
      await use({ url, bridge: bridge!.url, invoke, crawl: async () => {
        const field = page.getByLabel("Start URL", { exact: true });
        await field.fill(url);
        await field.press("Enter");
        await expect(page.getByTestId("crawl-phase")).toHaveText("Completed", { timeout: 30_000 });
        await expect(page.locator("main .os-data-grid")).toHaveAttribute("aria-rowcount", "4");
      } });
    } finally {
      await test.info().attach("bridge-stderr", { body: bridge?.stderr?.() ?? "", contentType: "text/plain" });
      await bridge?.stop();
      await new Promise<void>(resolve => server.close(() => resolve()));
    }
  },
});

export async function scan(page: Page, name: string, scope?: string): Promise<void> {
  const builder = new AxeBuilder({ page }).options({ resultTypes: ["violations"] });
  if (scope) builder.include(scope);
  const result = await builder.analyze();
  const evidence = { view: name, scope: scope ?? "document", engine: result.testEngine, violations: result.violations,
    passedRules: result.passes.map(rule => rule.id), incomplete: result.incomplete.map(rule => ({ id: rule.id, nodes: rule.nodes.length })) };
  await test.info().attach(`axe-${name}`, { body: JSON.stringify(evidence, null, 2), contentType: "application/json" });
  const findings = result.violations.filter(v => v.impact === "critical" || v.impact === "serious");
  // Only the recorded rule/targets may use the existing expected-failure mechanism.
  const knownTargets = name === "Crawl summary-issues-light"
    ? ['tr:nth-child(5) > td:nth-child(2) > .os-sidebar-priority-medium.os-sidebar-badge > span', 'tr:nth-child(5) > .os-sidebar-priority-medium']
    : name === "Crawl summary-segments-dark"
      ? ['.os-sidebar-actions > button[type="button"]', '.os-sidebar-empty > button[type="button"]'] : undefined;
  if (knownTargets && findings.length) {
    expect(findings.map(v => ({ rule: v.id, impact: v.impact, targets: v.nodes.map(n => n.target.join(" ")).sort() })), "Additional findings must remain unexpected").toEqual([
      { rule: "color-contrast", impact: "serious", targets: [...knownTargets].sort() },
    ]);
  }
  expect.soft(findings.map(v => ({ rule: v.id, impact: v.impact, nodes: v.nodes.map(n => ({ target: n.target, summary: n.failureSummary })) })), name).toEqual([]);
}

/** Reach the actual control by Tab; focus() is deliberately not used for reachability. */
export async function tabTo(page: Page, target: Locator, reverse?: boolean): Promise<void> {
  const backwards = reverse ?? await target.evaluate(el => !!(el.compareDocumentPosition(document.activeElement!) & Node.DOCUMENT_POSITION_FOLLOWING));
  const visited: string[] = [];
  for (let i = 0; i < 200; i++) {
    if (await target.evaluate(el => el === document.activeElement)) return;
    visited.push(await page.evaluate(() => { const e = document.activeElement; return `${e?.tagName} ${e?.getAttribute("role")} ${e?.getAttribute("aria-label")} ${e?.textContent?.slice(0, 70)}`; }));
    await page.keyboard.press(backwards ? "Shift+Tab" : "Tab");
  }
  await test.info().attach("tab-order-failure", { body: JSON.stringify(visited), contentType: "application/json" });
  await expect(target, "control must be reachable in the sequential keyboard order").toBeFocused();
}

export async function visibleFocus(target: Locator): Promise<void> {
  await expect(target).toBeFocused();
  const focus = await target.evaluate(el => {
    const style = getComputedStyle(el);
    const child = el.querySelector(".os-switch-track, .os-checkbox-box, .os-radio-dot");
    const s = child ? getComputedStyle(child) : style;
    return { visible: el.matches(":focus-visible") || el.hasAttribute("data-focus-visible"), outline: parseFloat(s.outlineWidth), style: s.outlineStyle, color: s.outlineColor, background: s.backgroundColor };
  });
  expect(focus.visible).toBe(true);
  expect(focus.outline).toBeGreaterThan(0);
  expect(focus.style).not.toBe("none");
  expect(focus.color).not.toBe(focus.background);
}

export async function selectFirstRow(page: Page): Promise<void> {
  const grid = page.locator("main .os-data-grid");
  await tabTo(page, grid);
  await grid.press("Home");
  await grid.press("ArrowDown");
  await expect(grid.getByRole("row", { selected: true })).toHaveCount(1);
  await expect(page.locator("#shell-detail")).toContainText("http://127.0.0.1:");
}

export async function shortcut(page: Page, key: string): Promise<void> {
  const modifier = await page.evaluate(async () => {
    const path = "/src/ui/platform.ts";
    const { currentPlatform } = await import(/* @vite-ignore */ path);
    return currentPlatform() === "mac" ? "Meta" : "Control";
  });
  await page.keyboard.press(`${modifier}+${key}`);
}

/** Walk the complete current sequential order (including long overview lists), without programmatic focus. */
export async function keyboardInventory(page: Page): Promise<void> {
  const expected = await page.evaluate(() => {
    const controls = [...document.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], summary, [tabindex]')]
      .filter(el => el.tabIndex >= 0 && !el.matches(':disabled') && !el.closest('[aria-hidden="true"], [inert]')
        && el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden")
      .filter(el => !(el instanceof HTMLInputElement && el.type === "radio" && !el.checked
        && [...document.querySelectorAll<HTMLInputElement>('input[type="radio"]')].some(r => r.name === el.name && r.checked)));
    return controls.map((el, i) => { el.dataset.u30Stop = String(i); return { id: String(i), name: el.getAttribute("aria-label") ?? el.textContent?.slice(0, 80) ?? el.tagName }; });
  });
  const seen = new Set<string>();
  for (let i = 0; i < expected.length + 15; i++) {
    const active = await page.evaluate(() => (document.activeElement as HTMLElement)?.dataset.u30Stop);
    if (active) seen.add(active);
    await page.keyboard.press("Tab");
  }
  const missing = expected.filter(stop => !seen.has(stop.id));
  await test.info().attach("keyboard-inventory", { body: JSON.stringify({ expected: expected.length, reached: seen.size, missing }), contentType: "application/json" });
  expect(missing, "every enabled sequential control must be reachable without a focus trap").toEqual([]);
}
