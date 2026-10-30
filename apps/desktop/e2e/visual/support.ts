import { expect, type Page } from "@playwright/test";
export const sizes = [{ width: 1024, height: 640 }, { width: 1440, height: 900 }, { width: 1920, height: 1080 }];
export async function prepare(page: Page, lang: string, theme: string, density = "compact", scene = "shell") {
  const platform = page.context().browser()?.browserType().name() === "webkit" ? "mac" : "win";
  await page.clock.setFixedTime(new Date("2026-10-01T12:00:00Z"));
  if (scene !== "shell") {
    await page.goto(`/?harness=${scene}&lang=${lang}&theme=${theme}&density=${density}&platform=${platform}`);
    await expect(page.locator("[data-harness-ready]")).toHaveAttribute("data-harness-ready", scene);
  } else {
    await page.goto(`/?harness=shell&lang=${lang}&theme=${theme}&density=${density}&platform=${platform}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async ({ lang, theme, density, platform }) => {
      const load = (path: string) => import(/* @vite-ignore */ path);
      const [{ setLanguage }, { usePrefs }, { applyAppearance }, { useLayout }] = await Promise.all([
        "/src/i18n/index.ts", "/src/state/prefs.ts", "/src/ui/tokens/theme.ts", "/src/state/layout.ts",
      ].map(load));
      await setLanguage(lang);
      const appearance = { ...usePrefs.getState().appearance, theme, density };
      usePrefs.setState({ appearance });
      applyAppearance(document.documentElement, appearance, { platform, systemDark: false });
      const s = useLayout.getState();
      s.setLayout({ ...s.layout, sidebar: { ...s.layout.sidebar, visible: true, collapsed: false, share: 200 / innerWidth }, detail: { ...s.layout.detail, visible: true, collapsed: false } });
    }, { lang, theme, density, platform });
    // The default dashboard has no top tab bar; the URL group exercises all three bars.
    await page.locator('.u34-nav [data-group="urls"] .u34-nav-button').click();
    await expect(page.locator('[role="tablist"]')).toHaveCount(3);
  }
  await page.evaluate(() => document.fonts.ready);
}

/** K2-75: inspect actual label nodes as well as their controls. Keep full diagnostics. */
export async function audit(page: Page) {
  return page.evaluate(() => {
    const visible = (e: HTMLElement) => {
      const r = e.getBoundingClientRect();
      return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== "hidden" && !e.closest('[aria-hidden="true"]');
    };
    const name = (e: HTMLElement) => `${e.tagName}.${e.className} [${e.getAttribute("role") ?? ""}] ${(e.getAttribute("aria-label") ?? e.textContent ?? "").trim()}`;
    const selector = 'button, a[href], input:not([type=hidden]), select, textarea, summary, [role=tab], [role=button], [role=combobox], [role=checkbox], [role^=menuitem], [role=separator][tabindex]';
    const controls = [...document.querySelectorAll<HTMLElement>(selector)].filter(visible);
    const labels = [...new Set([...controls, ...document.querySelectorAll<HTMLElement>('.os-tab-label, button span, [role^=menuitem] span, [role=dialog] h1, [role=dialog] h2, [role=alertdialog] h2')])].filter(visible);
    const text: string[] = [], outside: string[] = [], collisions: string[] = [];
    for (const e of labels) {
      if (!(e.textContent ?? "").trim()) continue;
      const cs = getComputedStyle(e);
      // Inline spans have zero client dimensions; their owning block/control is checked above.
      if (cs.display === "inline") continue;
      const full = (e.textContent ?? "").trim();
      const owner = e.closest<HTMLElement>('button, [role=tab], [role^=menuitem]') ?? e;
      const accessible = owner.getAttribute("aria-label") ?? owner.textContent?.trim();
      const tooltip = owner.getAttribute("title") ?? e.getAttribute("title");
      const ellipsis = cs.textOverflow === "ellipsis" && tooltip === full && accessible === full;
      if (e.scrollHeight > e.clientHeight || (e.scrollWidth > e.clientWidth && !ellipsis)) text.push(`${name(e)}: scroll=${e.scrollWidth}×${e.scrollHeight}, client=${e.clientWidth}×${e.clientHeight}, ellipsis=${ellipsis}`);
    }
    // A scroll container intentionally clips offscreen children. Test its visible intersection.
    const rect = (e: HTMLElement) => {
      const r = e.getBoundingClientRect(); let left = r.left, right = r.right, top = r.top, bottom = r.bottom;
      for (let p = e.parentElement; p; p = p.parentElement) {
        const s = getComputedStyle(p), b = p.getBoundingClientRect();
        if (s.overflowX !== "visible") { left = Math.max(left, b.left); right = Math.min(right, b.right); }
        if (s.overflowY !== "visible") { top = Math.max(top, b.top); bottom = Math.min(bottom, b.bottom); }
      }
      return { left, right, top, bottom };
    };
    for (const e of controls) {
      const r = rect(e);
      if (r.right <= r.left || r.bottom <= r.top) continue;
      if (r.left < 0 || r.top < 0 || r.right > innerWidth || r.bottom > innerHeight) outside.push(name(e));
    }
    for (let i = 0; i < controls.length; i++) for (const b of controls.slice(i + 1)) {
      const a = controls[i]!;
      if (a.contains(b) || b.contains(a)) continue;
      // Menus and modal dialogs intentionally cover the inactive background.
      const layer = (e: HTMLElement) => e.closest('[role=menu], [role=dialog], [role=alertdialog]');
      if (layer(a) !== layer(b)) continue;
      const x = rect(a), y = rect(b);
      if (Math.min(x.right, y.right) - Math.max(x.left, y.left) > 0 && Math.min(x.bottom, y.bottom) - Math.max(x.top, y.top) > 0) collisions.push(`${name(a)} × ${name(b)}`);
    }
    return { controls: controls.length, labels: labels.length, tabs: document.querySelectorAll('[role=tab]').length, text, outside, collisions };
  });
}
