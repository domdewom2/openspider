// OUT-246/OUT-247 E2E mit der echten Bridge (os-app-bridge) und einem Mock für /api/crash: Hinweis beim Start,
// „Bericht ansehen…“ zeigt Byte für Byte den gesendeten Bericht, Senden nur nach Klick, Fehler mit „Erneut versuchen“,
// „Nicht senden“ und Neustart ohne Verbindung, Hilfe → „Absturzberichte…“, Fehlergrenze und hartes Beenden.
import { spawn, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { bridgeBinary } from "../support/bridge";

const FIXTURE = join(import.meta.dirname, "..", "..", "..", "..", "tests", "fixtures", "crash", "crash-panic.json");

interface Mock { url: string; bodies: string[]; replies: Array<[number, string] | "hang">; close(): Promise<void> }

/** Mock für POST /api/crash: protokolliert jeden Körper (Netzwerkmitschnitt) und antwortet aus einer Warteschlange. */
async function mockServer(): Promise<Mock> {
  const bodies: string[] = [];
  const replies: Mock["replies"] = [];
  const server: Server = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => {
      bodies.push(Buffer.concat(chunks).toString("utf8"));
      const reply = replies.shift() ?? [200, '{"ok":true,"ref":"ABCD2345"}'];
      if (reply === "hang") return;
      res.writeHead(reply[0], { "Content-Type": "application/json" }).end(reply[1]);
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = (server.address() as { port: number }).port;
  return { url: `http://127.0.0.1:${port}/api/crash`, bodies, replies, async close() { server.closeAllConnections(); server.close(); } };
}

/** Startet die echte Bridge mit eigenem Home; `session` schaltet die Sitzungsmarke ein (wie die Desktop-App). */
async function bridge(home: string, endpoint: string, session = false) {
  const child: ChildProcess = spawn(bridgeBinary(), ["--home", home], {
    stdio: ["ignore", "pipe", "inherit"],
    env: { ...process.env, OPENSPIDER_SECRET_STORE: "memory", OPENSPIDER_CRASH_ENDPOINT: endpoint, ...(session ? { OPENSPIDER_CRASH_SESSION: "1" } : {}) },
  });
  const info = await new Promise<{ port: number; token: string }>((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => reject(new Error("Bridge startup timeout")), 20_000);
    child.once("exit", () => { clearTimeout(timer); reject(new Error("Bridge exited before startup")); });
    child.stdout!.on("data", (chunk: Buffer) => {
      output += chunk.toString();
      if (output.includes("\n")) { clearTimeout(timer); try { resolve(JSON.parse(output.split("\n")[0]!)); } catch (e) { reject(e); } }
    });
  });
  return {
    url: `ws://127.0.0.1:${info.port}/?token=${info.token}`,
    async stop(signal: NodeJS.Signals = "SIGTERM") { if (child.exitCode !== null || child.signalCode !== null) return; const exited = once(child, "exit"); child.kill(signal); await exited; },
  };
}

async function homeWithReport(): Promise<{ home: string; text: string }> {
  const home = await mkdtemp(join(tmpdir(), "os-crash-e2e-"));
  const dir = join(home, "logs", "crashes");
  await mkdir(dir, { recursive: true });
  const text = await readFile(FIXTURE, "utf8");
  await writeFile(join(dir, "crash-20261102-091500-panic.json"), text);
  return { home, text };
}

async function open(page: Page, url: string, lang: "de" | "en") {
  await page.goto(`/?bridge=${encodeURIComponent(url)}`);
  await expect.poll(() => page.evaluate(() => window.__osMenu?.model.lang)).toBeTruthy();
  await page.evaluate(async (lang) => {
    // @ts-expect-error Vite source module
    const { usePrefs } = await import("/src/state/prefs.ts");
    await usePrefs.getState().patch({ general: { language: lang } });
  }, lang);
  await expect.poll(() => page.evaluate(() => window.__osMenu?.model.lang)).toBe(lang);
}

const T = {
  de: { title: "OpenSpider hatte beim letzten Lauf ein Problem.", region: "Absturzbericht", view: "Bericht ansehen…", send: "Senden", dont: "Nicht senden", retry: "Erneut versuchen", close: "Schließen", json: "Inhalt des Berichts (wird genau so gesendet)", thanks: "Danke! Referenz: ABCD2345", failed: "Senden fehlgeschlagen: Der Server antwortete mit HTTP 500.", list: "Absturzberichte" },
  en: { title: "OpenSpider ran into a problem last time.", region: "Crash report", view: "View report…", send: "Send", dont: "Don't send", retry: "Try again", close: "Close", json: "Report content (sent exactly like this)", thanks: "Thank you! Reference: ABCD2345", failed: "Sending failed: The server answered with HTTP 500.", list: "Crash reports" },
} as const;

const serious = async (page: Page, selector: string) =>
  (await new AxeBuilder({ page }).include(selector).analyze()).violations.filter((v) => v.impact === "critical" || v.impact === "serious");

for (const lang of ["de", "en"] as const) {
  // spec: OUT-247
  test(`${lang}: hint, view shows exactly the sent report, send records the reference`, async ({ page }) => {
    const t = T[lang];
    const mock = await mockServer();
    const { home, text } = await homeWithReport();
    const core = await bridge(home, mock.url);
    try {
      await open(page, core.url, lang);
      const hint = page.getByRole("region", { name: t.region });
      await expect(hint.getByText(t.title)).toBeVisible();
      expect(await serious(page, ".os-crash-hint")).toEqual([]);
      await hint.getByRole("button", { name: t.view }).click();
      const dialog = page.getByRole("dialog", { name: t.region });
      const json = dialog.getByLabel(t.json);
      await expect(json).toHaveValue(text);
      const shown = await json.inputValue();
      expect(await serious(page, '[role="dialog"]')).toEqual([]);
      expect(mock.bodies).toHaveLength(0);
      await dialog.getByRole("button", { name: t.close, exact: true }).click();
      await hint.getByRole("button", { name: t.send, exact: true }).click();
      await expect(hint.getByText(t.thanks)).toBeVisible();
      expect(mock.bodies).toHaveLength(1);
      const body = mock.bodies[0]!;
      expect(body.slice('{"report":'.length, body.lastIndexOf(',"note":'))).toBe(shown);
      const stored = JSON.parse(await readFile(join(home, "logs", "crashes", "crash-20261102-091500-panic.json"), "utf8"));
      expect(stored.server_ref).toBe("ABCD2345");
      expect(typeof stored.sent).toBe("string");
    } finally {
      await core.stop();
      await mock.close();
      await rm(home, { recursive: true, force: true });
    }
  });
}

// spec: OUT-247
test("network capture: view and don't send, restart → no connection; failure, try again and the help list", async ({ page }) => {
  const t = T.en;
  const mock = await mockServer();
  const { home } = await homeWithReport();
  let core = await bridge(home, mock.url);
  try {
    await open(page, core.url, "en");
    const hint = page.getByRole("region", { name: t.region });
    await hint.getByRole("button", { name: t.view }).click();
    await page.getByRole("dialog", { name: t.region }).getByRole("button", { name: t.close, exact: true }).click();
    await hint.getByRole("button", { name: t.dont }).click();
    await expect(hint).toHaveCount(0);
    await core.stop();
    core = await bridge(home, mock.url);
    await open(page, core.url, "en");
    await expect(page.locator(".os-crash-hint")).toHaveCount(0);
    expect(mock.bodies).toHaveLength(0);
    // Hilfe → „Absturzberichte…“: der verworfene Bericht steht in der Liste und lässt sich senden.
    await expect.poll(() => page.evaluate(() => window.__osMenu?.model.menus.find((m) => m.id === "help")?.items.find((i) => i.action === "help.crashReports")?.enabled)).toBe(true);
    await page.evaluate(async () => {
      // @ts-expect-error Vite source module
      const { runAction } = await import("/src/ui/actions.ts");
      runAction("help.crashReports", { source: "menu" });
    });
    const list = page.getByRole("dialog", { name: t.list });
    await expect(list.getByRole("row")).toHaveCount(2);
    expect(await serious(page, '[role="dialog"]')).toEqual([]);
    mock.replies.push([500, '{"ok":false,"error":"server"}']);
    await list.getByRole("button", { name: t.send, exact: true }).click();
    await expect(list.getByText(t.failed)).toBeVisible();
    expect(mock.bodies).toHaveLength(1);
    await list.getByRole("button", { name: t.retry }).click();
    await expect(list.getByText(t.thanks)).toBeVisible();
    await expect(list.getByRole("cell", { name: "ABCD2345" })).toBeVisible();
    expect(mock.bodies).toHaveLength(2);
  } finally {
    await core.stop();
    await mock.close();
    await rm(home, { recursive: true, force: true });
  }
});

// spec: OUT-246
test("error boundary writes a local frontend report without any request; hard kill → unclean_exit hint after restart", async ({ page }) => {
  const mock = await mockServer();
  const home = await mkdtemp(join(tmpdir(), "os-crash-e2e-"));
  let core = await bridge(home, mock.url, true);
  try {
    await open(page, core.url, "en");
    await expect(page.locator(".os-crash-hint")).toHaveCount(0);
    await page.evaluate(async () => {
      // @ts-expect-error Vite source module
      const { useCrashProbe } = await import("/src/app/ErrorBoundary.tsx");
      useCrashProbe.setState({ region: "table" });
    });
    await expect(page.locator('[data-region-failed="table"]')).toBeVisible();
    const crashes = join(home, "logs", "crashes");
    await expect.poll(async () => (await readdir(crashes)).filter((n) => n.endsWith("-frontend.json")).length).toBe(1);
    // Berichte des laufenden Prozesses erscheinen erst beim nächsten Start.
    await expect(page.locator(".os-crash-hint")).toHaveCount(0);
    await core.stop("SIGKILL");
    core = await bridge(home, mock.url, true);
    await open(page, core.url, "en");
    const hint = page.getByRole("region", { name: T.en.region });
    await expect(hint.getByText(T.en.title)).toBeVisible();
    const kinds = (await readdir(crashes)).filter((n) => n.startsWith("crash-")).map((n) => n.replace(/^crash-\d{8}-\d{6}-/, "").replace(/(-\d+)?\.json$/, "")).sort();
    expect(kinds).toEqual(["frontend", "unclean_exit"]);
    expect(mock.bodies).toHaveLength(0);
  } finally {
    await core.stop();
    await mock.close();
    await rm(home, { recursive: true, force: true });
  }
});
