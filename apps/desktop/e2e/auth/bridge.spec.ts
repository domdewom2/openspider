import { chromium, expect, test, type Browser, type Page } from "@playwright/test";
import { createServer } from "node:http";
import { readdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startBridge } from "../support/bridge";
import type {} from "../config/harness";

// Connect only to a new login browser whose target matches this test's loopback origin.
async function loginBrowser(origin: string, previous: Set<string>): Promise<Browser> {
  let endpoint = "";
  await expect.poll(async () => {
    for (const dir of await readdir(tmpdir())) {
      if (!dir.startsWith("openspider-chromium-") || previous.has(dir)) continue;
      try {
        const [port, socket] = (await readFile(join(tmpdir(), dir, "DevToolsActivePort"), "utf8")).trim().split("\n");
        const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json() as { url: string }[];
        if (targets.some((t) => t.url.startsWith(origin))) {
          endpoint = `ws://127.0.0.1:${port}${socket}`;
          return true;
        }
      } catch { /* Chromium may still be starting. */ }
    }
    return false;
  }, { timeout: 20_000 }).toBe(true);
  return chromium.connectOverCDP(endpoint);
}
const configOf = (page: Page) => page.evaluate(() => window.cfgFixture.invoke("config.get"));

// spec: CRAWL-120
// spec: OUT-223
test("real bridge: capture, cancel configuration, reopen and activate only with OK", async ({ page }) => {
  test.setTimeout(90_000);
  const server = createServer((req, res) => {
    res.setHeader("Content-Type", "text/html");
    if (req.url === "/login") {
      res.end(`<!doctype html><form><label>User<input name="user"></label><label>Password<input name="password" type="password"></label><button>Sign in</button></form><script>
        document.querySelector('form').onsubmit=async e=>{e.preventDefault();await fetch('/session',{method:'POST'});localStorage.token='LOCAL-E2E';sessionStorage.token='SESSION-E2E';location.href='/members';};
      </script>`);
    } else if (req.url === "/session") {
      res.setHeader("Set-Cookie", "session=COOKIE-E2E; Path=/; HttpOnly; SameSite=Lax");
      res.end("ok");
    } else res.end("<!doctype html><title>Members</title>");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Loopback server unavailable");
  const origin = `http://127.0.0.1:${address.port}`;
  const previousHeadless = process.env.OPENSPIDER_LOGIN_HEADLESS;
  process.env.OPENSPIDER_LOGIN_HEADLESS = "1";
  const bridge = await startBridge();
  if (previousHeadless === undefined) delete process.env.OPENSPIDER_LOGIN_HEADLESS;
  else process.env.OPENSPIDER_LOGIN_HEADLESS = previousHeadless;
  try {
    expect(bridge, "real bridge required").not.toBeNull();
    await page.clock.install({ time: new Date("2026-10-27T00:00:00Z") });
    await page.goto(`/e2e/config/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.cfgFixture);
    expect(await page.evaluate(() => window.cfgFixture.transport)).toBe("bridge");
    const original = await configOf(page);
    for (const action of ["Cancel", "OK"]) {
      await page.evaluate(() => window.cfgFixture.open("auth.form"));
      const config = page.getByRole("dialog", { name: "Crawl configuration" });
      const previous = new Set(await readdir(tmpdir()));
      await config.getByRole("textbox", { name: "Login URL" }).fill(`${origin}/login`);
      await config.getByRole("button", { name: "Add and open browser" }).click();
      const browser = await loginBrowser(origin, previous);
      const login = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url().startsWith(origin))!;
      await login.getByRole("textbox", { name: "User" }).fill("fixture");
      await login.getByLabel("Password").fill("secret");
      await login.getByRole("button", { name: "Sign in" }).click();
      await expect(login).toHaveURL(`${origin}/members`);
      await page.getByRole("dialog", { name: "Form authentication" }).getByRole("button", { name: "Capture session" }).click();
      await expect(config.getByText("Signed in, 1 cookies")).toBeVisible();
      await expect(config.getByLabel("Session guard patterns")).toHaveValue(/log-\?out/);
      expect(await configOf(page)).toEqual(original);
      await config.getByRole("button", { name: action, exact: true }).click();
      await expect(config).not.toBeVisible();
      if (action === "Cancel") expect(await configOf(page)).toEqual(original);
      else {
        const saved = await configOf(page) as { config: { auth: { form: unknown[]; session_guard: string[] } } };
        expect(saved.config.auth.form).toHaveLength(1);
        expect(saved.config.auth.session_guard).toHaveLength(4);
        expect(JSON.stringify(saved)).not.toMatch(/COOKIE-E2E|LOCAL-E2E|SESSION-E2E/);
      }
      await browser.close();
    }
  } finally {
    await bridge?.stop();
    server.closeAllConnections();
    await new Promise<void>((resolve, reject) => server.close((e) => e ? reject(e) : resolve()));
  }
});
