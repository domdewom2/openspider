// Abnahme M2-U34 (5), Befund A02-04: Die deutsche Oberfläche zeigt keine englischen Resttexte. Der Durchlauf besucht alle Top-Tabs
// (Tab, Filter-Dropdown, Spaltenköpfe), alle Detail- und Seitenleisten-Tabs sowie die Überlauf- und Kontextmenüs der drei Leisten
// und meldet jeden sichtbaren Text, der einem englischen Katalogwert gleicht oder wie ein Schlüssel aussieht – außer der
// Ausnahmeliste aus docs/plan/M2-acceptance/beta/begriffe.md und Datenwerten (Tabellenzellen, URLs, Zahlen).
import { writeFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "../alpha/fixture";
import { BAR_LISTS, OVERFLOW, crawledWithSelection, exceptionTerms, showPanes, type Bar } from "./support";

test.use({ locale: "de-DE" });

interface Hit { text: string; where: string; zone: string }

/**
 * Bereiche der Oberfläche: `strict` gehört M2-U34 (Tab-Leisten, Menüs, Filter, Tabellenkopf und Werkzeugleiste der oberen
 * Tabelle, Rahmen von Detail- und Seitenleiste); alles andere wird gemeldet und im Bericht als „außerhalb owns“ geführt.
 */
async function sweep(page: Page, where: string, exceptions: string[]): Promise<Hit[]> {
  return page.evaluate(({ where, exceptions }) => {
    const w = window as unknown as { __osCatalog?: { english: string[] } };
    const english = new Set(w.__osCatalog!.english);
    const allowed = new Set(exceptions);
    const key = /^[a-z][a-z0-9]*(?:-[a-z][a-zA-Z0-9]*|-[0-9]+)+$/;
    const isData = (el: Element) => !!el.closest("[role=gridcell], td, pre, code, input, textarea, .os-mono, [data-value]");
    const zone = (el: Element): string => {
      if (el.closest("[role=menu], [role=listbox], [role=tablist], .u10-toolbar, .os-grid-a11y [aria-rowindex='1'], .os-detail-table-tools, .os-detail-heading, .os-tabbar")) return "strict";
      if (el.closest("header")) return "header";
      if (el.closest("footer")) return "statusbar";
      if (el.closest("#shell-detail")) return "detail-panel";
      if (el.closest("aside")) return "sidebar-panel";
      return "strict";
    };
    const report = (text: string, el: Element, out: Hit[]) => {
      const value = text.replace(/\s+/g, " ").trim();
      if (!value || allowed.has(value) || /^https?:\/\//.test(value) || /^[\d\s.,:%–\-+/()×#]+$/.test(value)) return;
      const parts = [value, ...value.split(/:\s+|\s+›\s+|\s+·\s+/)];
      for (const part of parts) {
        const p = part.trim();
        if (!p || allowed.has(p)) continue;
        if (english.has(p) || key.test(p)) { out.push({ text: p, where, zone: zone(el) }); return; }
      }
    };
    const hits: Hit[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const el = node.parentElement;
      if (!el || isData(el) || el.closest("[aria-hidden=true], script, style, datalist")) continue;
      if (el.getClientRects().length === 0 || getComputedStyle(el).visibility === "hidden") continue;
      report(node.textContent ?? "", el, hits);
    }
    for (const el of document.querySelectorAll<HTMLElement>("[aria-label], [title]")) {
      if (el.getClientRects().length === 0 || isData(el) || el.closest("[aria-hidden=true]")) continue;
      for (const attr of ["aria-label", "title"]) { const v = el.getAttribute(attr); if (v) report(v, el, hits); }
    }
    return hits;
  }, { where, exceptions });
}

/** Englische Katalogwerte, die im Deutschen anders lauten (einzeilige Werte ohne Platzhalter). */
async function loadCatalog(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const path = "/src/i18n/index.ts";
    const { loadFtlFiles } = await import(/* @vite-ignore */ path);
    const values = (lang: string) => {
      const map = new Map<string, string>();
      for (const file of loadFtlFiles().filter((f: { lang: string }) => f.lang === lang)) {
        for (const line of (file.source as string).split(/\r?\n/)) {
          const m = /^([a-zA-Z][\w-]*) = (.+)$/.exec(line);
          if (m && !m[2]!.includes("{")) map.set(m[1]!, m[2]!.trim());
        }
      }
      return map;
    };
    const en = values("en"); const de = values("de");
    const english = [...en].filter(([k, v]) => de.get(k) !== v && v.length > 1).map(([, v]) => v);
    (window as unknown as { __osCatalog: { english: string[] } }).__osCatalog = { english };
  });
}

async function openMenuAndSweep(page: Page, bar: Bar, exceptions: string[], hits: Hit[]): Promise<void> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  const root = list.locator("xpath=ancestor::div[contains(@class,'os-tabbar')][1]");
  await root.getByRole("button", { name: OVERFLOW[bar] }).click();
  await expect(page.getByRole("menu").last()).toBeVisible();
  hits.push(...await sweep(page, `${bar}: Überlaufmenü`, exceptions));
  await page.keyboard.press("Escape");
  await list.getByRole("tab").first().click({ button: "right" });
  await expect(page.getByRole("menu").last()).toBeVisible();
  hits.push(...await sweep(page, `${bar}: Kontextmenü`, exceptions));
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
}

/** Wählt einen Tab einer Leiste, auch wenn er nur im Überlaufmenü steht. */
async function selectTab(page: Page, bar: Bar, name: string): Promise<void> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  const tab = list.getByRole("tab", { name, exact: true });
  if (await tab.count()) { await tab.click(); return; }
  const root = list.locator("xpath=ancestor::div[contains(@class,'os-tabbar')][1]");
  await root.getByRole("button", { name: OVERFLOW[bar] }).click();
  await page.getByRole("menuitem", { name, exact: true }).click();
}

async function tabNames(page: Page, bar: Bar): Promise<string[]> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  const root = list.locator("xpath=ancestor::div[contains(@class,'os-tabbar')][1]");
  await root.getByRole("button", { name: OVERFLOW[bar] }).click();
  const names = await page.getByRole("menu").last().locator("[role=menuitemcheckbox]").evaluateAll((els) => els.map((e) => (e.textContent ?? "").replace(/^[☑☐]\s*/, "")));
  await page.keyboard.press("Escape");
  return names;
}

// spec: UI-121
// spec: UI-004
test("the German UI shows no English leftovers in tabs, filters, columns and the menus of the three tab bars (A02-04)", async ({ alpha }, testInfo) => {
  test.setTimeout(240_000);
  const { page } = alpha;
  const exceptions = exceptionTerms();
  await crawledWithSelection(alpha);
  await page.setViewportSize({ width: 1440, height: 900 });
  await showPanes(page, "default");
  await loadCatalog(page);
  const hits: Hit[] = [];
  for (const bar of ["top", "detail", "sidebar"] as const) await openMenuAndSweep(page, bar, exceptions, hits);
  for (const name of await tabNames(page, "top")) {
    await selectTab(page, "top", name);
    await expect(page.getByRole("tab", { name, exact: true, selected: true })).toBeVisible();
    await page.waitForTimeout(150);
    hits.push(...await sweep(page, `Top-Tab ${name}`, exceptions));
    await page.locator(".u10-toolbar .os-combobox-button").click();
    await expect(page.getByRole("listbox")).toBeVisible();
    hits.push(...await sweep(page, `Filter von ${name}`, exceptions));
    await page.keyboard.press("Escape");
    await expect(page.getByRole("listbox")).toHaveCount(0);
  }
  for (const bar of ["detail", "sidebar"] as const) {
    for (const name of await tabNames(page, bar)) {
      await selectTab(page, bar, name);
      await page.waitForTimeout(150);
      hits.push(...await sweep(page, `${bar === "detail" ? "Detail" : "Seitenleiste"}-Tab ${name}`, exceptions));
    }
  }
  const unique = [...new Map(hits.map((h) => [`${h.zone}|${h.text}`, h])).values()];
  const file = testInfo.outputPath("language-hits.json");
  writeFileSync(file, JSON.stringify(unique, null, 1));
  await testInfo.attach("language-hits", { path: file, contentType: "application/json" });
  if (process.env.OS_POLISH_HITS) writeFileSync(process.env.OS_POLISH_HITS, JSON.stringify(unique, null, 1));
  const strict = unique.filter((h) => h.zone === "strict");
  expect(strict, `englische Resttexte (Bereich M2-U34):\n${strict.map((h) => `${h.where}: ${h.text}`).join("\n")}`).toEqual([]);
});
