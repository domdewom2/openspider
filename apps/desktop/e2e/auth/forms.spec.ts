import { expect, test } from "@playwright/test";
import type {} from "../config/harness";

// The real form, 2FA, cookies and storage run through CDP in Rust integration tests.
// Here both web engines exercise the application's controls with metadata-only IPC.
// spec: CRAWL-120
test("captures a form session into configuration and discards a cancelled browser", async ({ page }) => {
  await page.goto("/e2e/config/index.html?transport=mock");
  await page.waitForFunction(() => !!window.cfgFixture);
  await page.evaluate(() => {
    const core = window.__osMock!;
    let saved = false;
    core.handlers.set("auth.openLogin", () => ({ opId: "form-login-op" }));
    core.handlers.set("auth.finishLogin", () => { saved = true; return { name: "fixture-profile", kind: "form", host: "fixture.test", createdAt: "2026-10-27T00:00:00Z", hasSecret: true }; });
    core.handlers.set("auth.loginStatus", () => saved ? [{ loginUrl: "https://fixture.test/login", state: "active", cookies: 1, capturedAt: "2026-10-27T00:00:00Z" }] : []);
    core.handlers.set("op.cancel", () => undefined);
    window.cfgFixture.open("auth.form");
  });
  const config = page.getByRole("dialog", { name: "Crawl configuration" });
  await config.getByRole("textbox", { name: "Login URL" }).fill("https://fixture.test/login");
  await config.getByRole("button", { name: "Add and open browser" }).click();
  const login = page.getByRole("dialog", { name: "Form authentication" });
  await login.getByRole("button", { name: "Capture session" }).click();
  await expect(config.getByText("Signed in, 1 cookies")).toBeVisible();
  await expect(config.getByLabel("Session guard patterns")).toHaveValue(/log-\?out/);
  await config.getByRole("textbox", { name: "Login URL" }).fill("https://fixture.test/another-login");
  await config.getByRole("button", { name: "Add and open browser" }).click();
  await login.getByRole("button", { name: "Cancel" }).click();
  await expect(login).not.toBeVisible();
  expect(await page.evaluate(() => window.cfgFixture.calls().filter((c) => c.cmd === "op.cancel").length)).toBe(1);
  await config.getByRole("button", { name: "OK", exact: true }).click();
  const saved = await page.evaluate(() => window.cfgFixture.saved());
  expect(JSON.stringify(saved)).toContain("fixture-profile");
  expect(JSON.stringify(saved)).not.toContain("COOKIE-FIXTURE");
});
