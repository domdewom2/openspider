import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "node:net";
import { defineConfig, devices } from "@playwright/test";

// Worktrees share a host: never reuse another package's fixed-port Vite server.
const port = process.env.OS_U21_E2E_PORT ? Number(process.env.OS_U21_E2E_PORT) : await new Promise<number>((resolve, reject) => {
  const server = createServer(); server.once("error", reject);
  server.listen(0, "127.0.0.1", () => {
    const address = server.address();
    if (!address || typeof address === "string") { server.close(); reject(new Error("Expected loopback port")); return; }
    server.close(error => error ? reject(error) : resolve(address.port));
  });
});
process.env.OS_U21_E2E_PORT = String(port);
export default defineConfig({
  testDir: import.meta.dirname,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  workers: 2,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: "list",
  outputDir: join(tmpdir(), "openspider-u21-e2e", String(port)),
  use: { baseURL: `http://127.0.0.1:${port}`, trace: "retain-on-failure", reducedMotion: "reduce" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }, { name: "webkit", use: { ...devices["Desktop Safari"] } }],
  webServer: { command: `npm run dev -- --host 127.0.0.1 --port ${port}`, url: `http://127.0.0.1:${port}`, reuseExistingServer: false, timeout: 60_000 },
});
