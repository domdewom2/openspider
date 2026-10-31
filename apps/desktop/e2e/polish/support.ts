// Hilfen der Polish-Abnahme M2-U34 (Befunde A02-01 … A02-06): echte Bridge und Fixture-Site wie die Alpha-Abnahme, dazu Messung der
// drei Tab-Leisten, Öffnen ihrer Überlauf- und Kontextmenüs und das Einstellen der Seitenleistenbreite.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, type Alpha } from "../alpha/fixture";

import { withTextGeometry } from "../n12/textGeometry";

export type Bar = "top" | "detail" | "sidebar";
export const SIZES = [
  { width: 1024, height: 640 },
  { width: 1440, height: 900 },
  { width: 1920, height: 1080 },
] as const;

/** Die drei Leisten über ihre Tablist (Namen de/en). */
export const BAR_LISTS: Record<Bar, RegExp> = {
  top: /^(Daten-Tabs|Data tabs)$/,
  detail: /^(Detail-Tabs|Detail tabs)$/,
  sidebar: /^(Crawl-Zusammenfassung|Crawl summary)$/,
};
/** Name des Überlaufknopfs je Leiste (de/en). */
export const OVERFLOW: Record<Bar, RegExp> = {
  top: /^(Tabs konfigurieren|Configure tabs)$/,
  detail: /^(Weitere Detail-Tabs|More detail tabs)$/,
  sidebar: /^(Weitere Seitenleisten-Tabs|More sidebar tabs)$/,
};
export const CONFIGURE = /^(Tabs konfigurieren…|Configure tabs…)$/;
export const RESET = /^(Tabs zurücksetzen|Reset tabs)$/;

/** Crawl der Fixture-Site über die Kopfleiste; danach eine URL für das Detailfenster auswählen. */
export async function crawledWithSelection(alpha: Alpha): Promise<void> {
  await alpha.open();
  const field = alpha.page.getByLabel(/^(Start URL|Start-URL)$/);
  await field.fill(alpha.startUrl);
  await field.press("Enter");
  await expect(alpha.page.getByTestId("crawl-phase")).toHaveText(/^(Completed|Fertig)$/, { timeout: 30_000 });
  await alpha.page.evaluate(async () => {
    const viewPath = "/src/ipc/view.ts";
    const selectionPath = "/src/features/detail/frame/selection.ts";
    const { view } = await import(/* @vite-ignore */ viewPath);
    const { setDetailSelection } = await import(/* @vite-ignore */ selectionPath);
    for (let attempt = 0; attempt < 15; attempt++) {
      try {
        const handle = await view.open({ spec: { kind: "tab", tab: "Internal", filter: "All", search: null, sort: [] }, columns: ["Address"] });
        const rows = await view.rows({ viewId: handle.viewId, version: handle.version, offset: 0, limit: 1, columns: ["Address"] });
        await view.close({ viewId: handle.viewId });
        const id = rows.rowKeys[0] as number;
        setDetailSelection({ urlIds: [id], focus: id, topTab: "Internal" });
        return;
      } catch {
        await new Promise((r) => setTimeout(r, 300));
      }
    }
    throw new Error("keine URL für das Detailfenster");
  });
}

/** Zeigt die Seitenleiste mit `width` px (UI-001: Mindestbreite 200 px) und das Detailfenster. */
export async function showPanes(page: Page, sidebarWidth: number | "default"): Promise<void> {
  await page.evaluate(async (sidebarWidth) => {
    const path = "/src/state/layout.ts";
    const { useLayout } = await import(/* @vite-ignore */ path);
    const s = useLayout.getState();
    const share = sidebarWidth === "default" ? Math.max(260, window.innerWidth * 0.25) / window.innerWidth : sidebarWidth / window.innerWidth;
    s.setLayout({ ...s.layout, sidebar: { visible: true, collapsed: false, share }, detail: { ...s.layout.detail, visible: true, collapsed: false } });
  }, sidebarWidth);
  await expect(page.locator("aside#shell-sidebar")).toBeVisible();
}

export interface TabMeasure {
  id: string;
  text: string;
  title: string | null;
  verticalCut: boolean;
  horizontalCut: boolean;
  /** Tab ragt über die Leiste hinaus oder schneidet den Überlaufknopf. */
  overlapsTrigger: boolean;
  outsideList: boolean;
}

/** Misst alle sichtbaren Tabs einer Leiste (Kriterium 1). */
export async function measureBar(page: Page, bar: Bar): Promise<{ tabs: TabMeasure[]; hasTrigger: boolean }> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  await expect(list).toBeVisible();
  return list.evaluate(withTextGeometry((el: HTMLElement | SVGElement, textGeometry) => {
    const root = el.closest(".os-tabbar") as HTMLElement;
    const trigger = root.querySelector<HTMLElement>(":scope > [data-tabbar-overflow]");
    const t = trigger?.getBoundingClientRect();
    const l = el.getBoundingClientRect();
    const tabs = [...el.querySelectorAll<HTMLElement>("[role=tab]")].filter((tab) => !tab.hidden && getComputedStyle(tab).visibility !== "hidden");
    const geometry = textGeometry(tabs.map(tab => tab.querySelector(".os-tab-label")!));
    return {
      hasTrigger: !!trigger,
      tabs: tabs.map((tab, index) => {
        const r = tab.getBoundingClientRect();
        const overlaps = !!t && r.left < t.right - 0.5 && r.right > t.left + 0.5 && r.top < t.bottom && r.bottom > t.top;
        return {
          id: tab.dataset.tabId ?? "",
          text: tab.textContent ?? "",
          title: tab.getAttribute("title"),
          verticalCut: geometry[index]!.vertical,
          horizontalCut: geometry[index]!.horizontal,
          overlapsTrigger: overlaps,
          outsideList: r.right > l.right + 0.5 || r.left < l.left - 0.5,
        };
      }),
    };
  }));
}

/** Öffnet das Überlaufmenü einer Leiste; gibt die Einträge zurück und schließt es wieder. */
export async function overflowMenu(page: Page, bar: Bar): Promise<{ checkboxes: { name: string; checked: boolean }[]; items: string[] }> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  const trigger = list.locator("xpath=ancestor::div[contains(@class,'os-tabbar')][1]").getByRole("button", { name: OVERFLOW[bar] });
  await trigger.click();
  const menu = page.getByRole("menu").last();
  await expect(menu).toBeVisible();
  const result = await menu.evaluate((m) => ({
    checkboxes: [...m.querySelectorAll<HTMLElement>("[role=menuitemcheckbox]")].map((e) => ({ name: (e.textContent ?? "").replace(/^[☑☐]\s*/, ""), checked: e.getAttribute("aria-checked") === "true" })),
    items: [...m.querySelectorAll<HTMLElement>("[role=menuitem]")].map((e) => e.textContent ?? ""),
  }));
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  return result;
}

/** Einträge des Kontextmenüs eines Tabs (UI-112). */
export async function contextMenu(page: Page, bar: Bar): Promise<string[]> {
  const list = page.getByRole("tablist", { name: BAR_LISTS[bar] });
  await list.getByRole("tab").first().click({ button: "right" });
  const menu = page.getByRole("menu").last();
  await expect(menu).toBeVisible();
  const items = await menu.evaluate((m) => [...m.querySelectorAll<HTMLElement>("[role=menuitem]")].map((e) => e.textContent ?? ""));
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  return items;
}

/** Ausnahmeliste (a)–(c) aus begriffe.md: Spalte 1 der Tabelle „Ausnahmen“, in Backticks. */
export function exceptionTerms(): string[] {
  const file = join(import.meta.dirname, "..", "..", "..", "..", "docs", "plan", "M2-acceptance", "beta", "begriffe.md");
  const text = readFileSync(file, "utf8");
  const section = text.split(/^## /m).find((s) => s.startsWith("Ausnahmen")) ?? "";
  return [...section.matchAll(/^\|\s*`([^`]+)`/gm)].map((m) => m[1]!);
}

/** Gruppe eines Top-Tabs nach UI-004 (K2-77), aus derselben Zuordnung wie die Oberfläche. */
export async function groupOfTab(page: Page, tab: string): Promise<string> {
  return page.evaluate(async (tab) => {
    const path = "/src/state/tabs.ts";
    const { groupOf } = await import(/* @vite-ignore */ path);
    return groupOf(tab) as string;
  }, tab);
}

/** Alle Top-Tabs des Katalogs (kanonische Namen) und ihre Anzeige in der Oberflächensprache. */
export async function catalogTabs(page: Page): Promise<{ id: string; label: string; group: string }[]> {
  return page.evaluate(async () => {
    const [{ catalog }, { groupOf }, { getT }, { tabLabel }] = await Promise.all(["/src/ipc/catalog.ts", "/src/state/tabs.ts", "/src/i18n/index.ts", "/src/i18n/canon.ts"].map((p) => import(/* @vite-ignore */ p)));
    const t = getT();
    return ((await catalog.tabs()) as { id: string; defaultOrder: number }[]).sort((a, b) => a.defaultOrder - b.defaultOrder)
      .map((tab) => ({ id: tab.id, label: tabLabel(t, tab.id) as string, group: groupOf(tab.id) as string }));
  });
}

/** Navigationsleiste der Gruppen (K2-77). */
export const navBar = (page: Page) => page.getByRole("group", { name: /^(Bereiche|Areas)$/ });
