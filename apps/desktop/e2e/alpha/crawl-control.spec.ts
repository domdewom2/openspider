// Alpha-Kriterium 1 (docs/plan/M2.md 4.3): Start-URL von fx-ui-basic → Crawl läuft; Kopf- und Statusleiste zeigen Zustand, Raten,
// Erledigt/Rest und Speicher; Pause/Fortsetzen/Stopp wirken; höchstens 10 Ticks je Sekunde. Echte Bridge, echter Kern.
import type { Page } from "@playwright/test";
import { expect, test } from "./fixture";

const status = (page: Page) => page.getByTestId("crawl-status");
const button = (page: Page, name: string) => page.getByRole("button", { name, exact: true });
/** „Done A of B (P %)“ → [A, B, P]. */
async function done(page: Page): Promise<[number, number, number]> {
  const m = /Done ([\d,]+) of ([\d,]+) \((\d+) %\)/.exec(await status(page).innerText());
  if (!m) throw new Error("Zähler „Done A of B“ fehlt in der Statusleiste");
  return [Number(m[1]!.replace(/,/g, "")), Number(m[2]!.replace(/,/g, "")), Number(m[3])];
}

// spec: UI-003
// spec: UI-013
test("start, running, pause, resume and completion drive the header buttons and the status bar", async ({ alpha }) => {
  test.setTimeout(90_000);
  const { page } = alpha;
  await alpha.open();
  await alpha.slowCrawl(3);

  // leer → Start gesperrt, nichts zu pausieren oder zu leeren
  await expect(page.getByTestId("crawl-phase")).toHaveText("Ready");
  for (const name of ["Start", "Pause", "Clear"]) await expect(button(page, name)).toBeDisabled();

  // Enter im URL-Feld = Start; läuft → Pause aktiv, Start und Löschen gesperrt
  await alpha.startFromHeader();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
  await expect(button(page, "Start")).toBeDisabled();
  await expect(button(page, "Pause")).toBeEnabled();
  await expect(button(page, "Clear")).toBeDisabled();

  // Statusleiste: Modus, Raten, Erledigt/Rest, Threads, Speicher, Crawl-Datei – mit echten Werten
  await expect.poll(async () => (await done(page))[0], { timeout: 15_000 }).toBeGreaterThanOrEqual(4);
  await expect(status(page)).toContainText("Mode: Spider");
  await expect(status(page)).toContainText(/Current: [\d.,]+ URL\/s/);
  await expect(status(page)).toContainText(/Average: [\d.,]+ URL\/s/);
  await expect(status(page)).toContainText(/Remaining: [1-9]\d*/);
  await expect(status(page)).toContainText(/Threads: [1-9]/);
  await expect(status(page)).toContainText(/Memory: [\d.,]+ \w+ \/ [\d.,]+ \w+/);
  await expect(status(page)).toContainText(/Crawl file: [\d.,]+ \w+/);
  const bar = page.getByRole("progressbar", { name: "Crawl progress" });
  const percent = Number(await bar.getAttribute("aria-valuenow"));
  expect(percent).toBeGreaterThan(0);
  expect(percent).toBeLessThan(100);

  // Pause wirkt: Zustand „Paused“, Fortsetzen und Löschen aktiv, Zähler stehen still
  await button(page, "Pause").click();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Paused");
  await expect(button(page, "Resume")).toBeEnabled();
  await expect(button(page, "Clear")).toBeEnabled();
  await expect(button(page, "Start")).toBeDisabled();
  await page.waitForTimeout(1500);
  const paused = await done(page);
  await page.waitForTimeout(1500);
  expect(await done(page)).toEqual(paused);

  // Fortsetzen wirkt: läuft weiter und endet vollständig
  await button(page, "Resume").click();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
  await expect.poll(async () => (await done(page))[0], { timeout: 15_000 }).toBeGreaterThan(paused[0]);
  await alpha.waitCompleted();
  expect(await done(page)).toEqual([27, 27, 100]);
  await expect(status(page)).toContainText("Remaining: 0");
  await expect(button(page, "Start")).toBeEnabled();
  await expect(button(page, "Clear")).toBeEnabled();
  await expect(button(page, "Pause")).toBeDisabled();

  // Löschen leert die Ansicht ohne Rückfrage
  await button(page, "Clear").click();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Ready");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(button(page, "Clear")).toBeDisabled();
});

// spec: UI-003
test("Stop ends a running crawl: no further progress and no running state", async ({ alpha }) => {
  test.setTimeout(60_000);
  const { page } = alpha;
  await alpha.open();
  await alpha.slowCrawl(3);
  await alpha.startFromHeader();
  await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
  await expect.poll(async () => (await done(page))[0], { timeout: 15_000 }).toBeGreaterThanOrEqual(3);
  // Die Oberfläche hat kein Bedienelement für `crawl.stop` (Befund A01-05); der Befehl ist trotzdem Teil des Vertrags (UI-003, CRAWL-126).
  await alpha.invoke("crawl.stop");
  await expect(page.getByTestId("crawl-phase")).not.toHaveText("Running");
  await expect(button(page, "Pause")).toBeDisabled();
  const stopped = await done(page);
  await page.waitForTimeout(2000);
  expect(await done(page)).toEqual(stopped);
});

// spec: UI-013
test("a stopped crawl never reports 100 % while URLs remain in the queue", async ({ alpha }) => {
  test.fixme(true, "Befund A01-06 (docs/plan/M2-acceptance/alpha/befunde.md): nach crawl.stop zeigt die Statusleiste „Completed“ und 100 % bei Remaining > 0 (UI-013: 100 % erst bei leerer Warteschlange)");
  const { page } = alpha;
  await alpha.open();
  await alpha.slowCrawl(3);
  await alpha.startFromHeader();
  await expect.poll(async () => (await done(page))[0], { timeout: 15_000 }).toBeGreaterThanOrEqual(3);
  await alpha.invoke("crawl.stop");
  await expect(page.getByTestId("crawl-phase")).not.toHaveText("Running");
  const [crawled, total, pct] = await done(page);
  expect(crawled).toBeLessThan(total);
  expect(pct).toBeLessThan(100);
});

/** Zählt Tick-Ereignisse auf der Leitung (Bridge → Seite) und liefert die größte Zahl je 1-Sekunden-Fenster. */
function tickCounter(page: Page): { stamps: number[]; maxPerSecond(): number } {
  const stamps: number[] = [];
  page.on("websocket", (ws) => {
    ws.on("framereceived", (frame) => {
      if (String(frame.payload).includes('"event":"tick"')) stamps.push(Date.now());
    });
  });
  return {
    stamps,
    maxPerSecond() {
      let best = 0;
      for (let i = 0; i < stamps.length; i++) {
        let j = i;
        while (j < stamps.length && stamps[j]! - stamps[i]! < 1000) j++;
        best = Math.max(best, j - i);
      }
      return best;
    },
  };
}

// spec: UI-018
test("the core delivers at most 10 UI updates per second at the default interval", async ({ alpha }) => {
  test.setTimeout(60_000);
  const ticks = tickCounter(alpha.page);
  await alpha.open();
  await alpha.slowCrawl(3);
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  expect(ticks.stamps.length, "es müssen Ticks gemessen worden sein").toBeGreaterThanOrEqual(5);
  expect(ticks.maxPerSecond()).toBeLessThanOrEqual(10);
  // Standardintervall 500 ms: im Mittel höchstens gut zwei gebündelte Ereignisse je Sekunde
  const seconds = (ticks.stamps.at(-1)! - ticks.stamps[0]!) / 1000;
  expect(ticks.stamps.length / Math.max(seconds, 1)).toBeLessThanOrEqual(4);
});

// spec: UI-018
test("the hard limit of 10 updates per second also holds at the shortest interval of 250 ms", async ({ alpha }) => {
  test.setTimeout(60_000);
  const ticks = tickCounter(alpha.page);
  await alpha.open();
  await alpha.slowCrawl(6);
  await alpha.page.evaluate(async () => {
    const path = "/src/state/prefs.ts";
    const { usePrefs } = await import(/* @vite-ignore */ path);
    await usePrefs.getState().patch({ appearance: { liveIntervalMs: 250 } });
  });
  await alpha.startFromHeader();
  await alpha.waitCompleted();
  expect(ticks.stamps.length).toBeGreaterThanOrEqual(5);
  expect(ticks.maxPerSecond()).toBeLessThanOrEqual(10);
});
