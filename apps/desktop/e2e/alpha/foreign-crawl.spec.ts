// Alpha-Kriterium 6 (docs/plan/M2.md 4.3, F-14, AGENT-020): Ein per CLI gestarteter Crawl derselben Maschine erscheint live in der App,
// nur lesend. Die CLI läuft im selben Datenordner wie die Bridge; die App öffnet den Crawl aus der Crawl-Verwaltung.
import type { Page } from "@playwright/test";
import { expect, test, type Alpha } from "./fixture";
import { spawnCli } from "./support";

const status = (page: Page) => page.getByTestId("crawl-status");
async function crawled(page: Page): Promise<number> {
  const m = /Done ([\d,]+) of ([\d,]+) \((\d+) %\)/.exec(await status(page).innerText());
  return m ? Number(m[1]!.replace(/,/g, "")) : -1;
}

/** Startet den gedrosselten CLI-Crawl und öffnet ihn über „Crawls verwalten…“ per Doppelklick. */
async function startForeignCrawl(alpha: Alpha) {
  const { page } = alpha;
  await alpha.open();
  const cli = spawnCli(["--crawl", alpha.startUrl, "--set", "speed.max_urls_per_sec=3", "--set", "speed.threads=1", "--save-crawl", "--output-folder", `${alpha.bridge.home}/cli-output`], { home: alpha.bridge.home });
  // die Crawl-Verwaltung sieht den laufenden Crawl (Registry + Heartbeat)
  await expect
    .poll(async () => (await alpha.invoke<{ crawl: { state?: string } }[]>("crawls.list", { show: "all" })).length, { timeout: 20_000 })
    .toBeGreaterThan(0);
  await page.evaluate(async () => {
    const path = "/src/ui/actions.ts";
    const { runAction } = await import(/* @vite-ignore */ path);
    runAction("crawls.manage", { source: "menu" });
  });
  const row = page.getByRole("table", { name: "Saved crawls" }).getByRole("row").nth(1);
  await expect(row).toContainText("Running");
  await row.dblclick();
  return cli;
}

// spec: UI-013
test("a crawl started by the CLI appears live in the app and cannot be controlled from it", async ({ alpha }) => {
  test.setTimeout(120_000);
  const { page } = alpha;
  const cli = await startForeignCrawl(alpha);
  try {
    // live: Zustand „Running“, Zähler wachsen, Raten kommen aus dem Fortschrittsstand des fremden Prozesses
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    const first = await crawled(page);
    await expect.poll(() => crawled(page), { timeout: 20_000 }).toBeGreaterThan(first);
    await expect(status(page)).toContainText(/Current: [\d.,]+ URL\/s/);
    await expect(status(page)).toContainText(/Threads: 1/);

    // nur lesend: alle Steuerelemente gesperrt, Hinweis auf den fremden Prozess, Befehle werden vom Kern abgelehnt
    await expect(page.getByRole("button", { name: "Start", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Clear", exact: true })).toBeDisabled();
    await expect(page.getByText("This crawl is controlled by another process.")).toBeVisible();
    for (const [cmd, args] of [["crawl.pause", undefined], ["crawl.stop", undefined], ["crawl.resume", undefined], ["crawl.remove", { selection: { kind: "ids", ids: [1] } }]] as const) {
      await expect(alpha.invoke(cmd, args), cmd).rejects.toThrow();
    }

    // die Tabelle wächst mit dem fremden Crawl
    await alpha.selectTab("Response Codes");
    const rows = async () => Number(/Showing: (\d+) of/.exec(await alpha.counter().catch(() => ""))?.[1] ?? -1);
    const early = await rows();
    await expect.poll(rows, { timeout: 20_000 }).toBeGreaterThan(early);

    // der CLI-Crawl wird nicht gestört: er endet regulär, die App zeigt „Completed“ mit allen URLs
    const result = await cli.done;
    expect(result.code, result.stderr).toBe(0);
    await alpha.waitCompleted(30_000);
    await expect.poll(() => crawled(page), { timeout: 15_000 }).toBe(27);
    await expect(status(page)).toContainText("Remaining: 0");
  } finally {
    cli.child.kill();
  }
});

// Alpha 6, Herzschlag des fremden Prozesses (F-14, CRAWL-127): fremde ID, kein Spec-Tag.
test("an interrupted foreign crawl is shown as interrupted, not as running", async ({ alpha }) => {
  test.setTimeout(120_000);
  const { page } = alpha;
  const cli = await startForeignCrawl(alpha);
  await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
  await expect.poll(() => crawled(page), { timeout: 20_000 }).toBeGreaterThan(0);
  // Der Prozess verschwindet ohne Abschluss (Absturz/Kill): der Herzschlag bleibt aus, die Anzeige fällt von „Running“ zurück
  cli.child.kill("SIGKILL");
  await cli.done;
  await expect(page.getByTestId("crawl-phase")).not.toHaveText("Running", { timeout: 30_000 });
  await expect(page.getByTestId("crawl-phase")).toHaveText("Interrupted");
});
