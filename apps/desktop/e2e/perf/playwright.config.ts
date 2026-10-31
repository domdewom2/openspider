import { createServer } from "node:net";
import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";
const port = process.env.OS_PERF_PREVIEW_PORT
  ? Number(process.env.OS_PERF_PREVIEW_PORT)
  : await new Promise<number>((resolvePort, reject) => {
      const server = createServer();
      server.on("error", reject);
      server.listen(0, "127.0.0.1", () => {
        const address = server.address();
        if (!address || typeof address === "string")
          return reject(new Error("No preview port"));
        server.close(() => resolvePort(address.port));
      });
    });
if (!Number.isInteger(port) || port < 1 || port > 65535)
  throw new Error("Invalid preview port");
// Playwright reloads this config in workers; inherit the coordinator's port.
process.env.OS_PERF_PREVIEW_PORT = String(port);
const baseURL = `http://127.0.0.1:${port}`;
const engines = (process.env.OS_E2E_PROJECTS ?? "chromium,webkit").split(",");
export default defineConfig({
  testDir: ".",
  testMatch: "*.bench.ts",
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 180_000,
  reporter: [
    ["list"],
    [
      "json",
      {
        outputFile: resolve(
          import.meta.dirname,
          "../../../../target/loadtest/playwright.json",
        ),
      },
    ],
    [
      "junit",
      {
        outputFile: resolve(
          import.meta.dirname,
          "../../../../target/loadtest/junit.xml",
        ),
      },
    ],
  ],
  outputDir: resolve(import.meta.dirname, "../../../../target/ui-perf-tests"),
  expect: { timeout: 30_000 },
  use: {
    baseURL,
    viewport: { width: 1280, height: 800 },
  },
  projects: engines.map((name) => ({
    name,
    use: {
      browserName: name as "chromium" | "webkit",
      ...(name === "chromium" && process.env.OPENSPIDER_CHROMIUM
        ? { launchOptions: { executablePath: process.env.OPENSPIDER_CHROMIUM } }
        : {}),
    },
  })),
  webServer: {
    command: `npx vite preview --config e2e/perf/vite.config.ts --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
