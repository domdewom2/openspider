import { createServer } from "node:http";
import { test, expect } from "../fixtures/app";
import tick from "../../src/features/header/fixtures/tick-running.json" with { type: "json" };

// spec: UI-003
test("validates start addresses inline and shows the subfolder slash hint", async ({ app, page }) => {
  await app.open(page, "main");
  const url = page.getByLabel("Start URL");
  const start = page.getByRole("button", { name: "Start", exact: true });
  await expect(start).toBeDisabled();
  for (const value of ["ht tp://x", "javascript:alert(1)", "https://ex ample.test"]) {
    await url.fill(value); await expect(url).toHaveValue(value); await expect(start, value).toBeDisabled(); await expect(url).toHaveAttribute("aria-invalid", "true");
  }
  await url.fill("127.0.0.1:8123/blog");
  await expect(start).toBeEnabled();
  await page.getByLabel("Scope").selectOption("folder");
  await expect(page.getByText(/Without a trailing slash/)).toBeVisible();
  await url.fill("http://127.0.0.1:8123/blog/");
  await expect(page.getByText(/Without a trailing slash/)).toHaveCount(0);
});

// spec: CRAWL-001 [M2]
test("starts, pauses, resumes and clears a saved crawl through the bridge contract", async ({ app, page }) => {
  // N01-E2E-2: crawl.pause waits for the in-flight requests and the durable checkpoint (CRAWL-126: < 3 s even with a 1M queue).
  // Measured on macOS arm64 (debug bridge): 0.12 s without load, 0.9-2.3 s while the UI polls; the CI runners are slower, so the
  // phase expectations after pause/resume get 20 s instead of the default 5 s. The assertions are unchanged.
  test.setTimeout(90_000);
  const slow = { timeout: 20_000 };
  const server = createServer((req, res) => {
    setTimeout(() => {
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end(req.url === "/" ? Array.from({ length: 100 }, (_, i) => `<a href="/p/${i}">${i}</a>`).join("") : "<title>Fixture page</title>");
    }, 200);
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Fixture address missing");
  try {
    await app.open(page, "main");
    if (app.mode === "mock") await page.evaluate((base) => {
      const core = window.__osMock!;
      let seq = 1;
      const emit = (phase: "running" | "paused" | "idle") => {
        core.emit("main", { kind: "tick", tick: { ...base, seq: seq++, crawl: { ...base.crawl, phase } } } as never);
      };
      core.handlers.set("crawl.validateStartUrl", (args) => ({ normalized: (args as { input: string }).input }));
      core.handlers.set("crawl.start", () => { emit("running"); return { crawlId: "fixture", file: "saved.ospider" }; });
      core.handlers.set("crawl.pause", () => emit("paused"));
      core.handlers.set("crawl.resume", () => emit("running"));
      core.handlers.set("crawl.clear", () => emit("idle"));
    }, tick);
    await page.getByLabel("Start URL").fill(`http://127.0.0.1:${address.port}/`);
    await page.getByLabel("Start URL").press("Enter");
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    await expect(page.getByRole("button", { name: "Start", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(page.getByTestId("crawl-phase")).toHaveText("Paused", slow);
    await page.getByRole("button", { name: "Resume", exact: true }).click();
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running", slow);
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await page.getByRole("button", { name: "Clear", exact: true }).click();
    await expect(page.getByTestId("crawl-phase")).toHaveText("Ready", slow);
    await expect(page.getByRole("dialog")).toHaveCount(0);
  } finally { await new Promise<void>((resolve, reject) => server.close((e) => e ? reject(e) : resolve())); }
});
