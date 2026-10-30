// Isolated server: another package runs E2E in a neighbouring worktree on this host.
import { createServer } from "node:net";
import { defineConfig, devices } from "@playwright/test";

async function vitePort(): Promise<number> {
  if (process.env.OS_U30_VITE_PORT) return Number(process.env.OS_U30_VITE_PORT);
  const reservation = createServer();
  await new Promise<void>(resolve => reservation.listen(0, "127.0.0.1", resolve));
  const address = reservation.address();
  if (!address || typeof address === "string") throw new Error("No Vite port");
  const port = address.port;
  await new Promise<void>(resolve => reservation.close(() => resolve()));
  process.env.OS_U30_VITE_PORT = String(port); // Workers re-import this config and must use the same server.
  return port;
}
const port = await vitePort();

export default defineConfig({
  testDir: "..",
  testIgnore: /(?:viz[\\/]perf\.spec\.ts|visual[\\/])/,
  fullyParallel: true,
  workers: 4,
  timeout: 120_000,
  expect: { timeout: 10_000 },
  outputDir: "../../../../target/u30-playwright",
  reporter: [["list"], ["json", { outputFile: "../../../../target/u30-playwright.json" }]],
  use: { baseURL: `http://127.0.0.1:${port}`, trace: "retain-on-failure", reducedMotion: "reduce" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
    { name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1280, height: 800 } } },
  ],
  webServer: {
    command: `npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
    timeout: 60_000,
    stdout: "ignore",
  },
});
