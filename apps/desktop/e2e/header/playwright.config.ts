import { createServer } from "node:net";
import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";
import base from "../../playwright.config";

// Independent worktrees can run browser tests concurrently; reserve an ephemeral local Vite port.
async function localPort(): Promise<string> {
  if (process.env.U03_E2E_PORT) return process.env.U03_E2E_PORT;
  const probe = createServer();
  await new Promise<void>((resolve) => probe.listen(0, "127.0.0.1", resolve));
  const address = probe.address();
  if (!address || typeof address === "string") throw new Error("Missing local test port");
  const port = String(address.port);
  await new Promise<void>((resolve, reject) => probe.close((e) => e ? reject(e) : resolve()));
  // Workers re-evaluate the config; inherit the coordinator's port.
  process.env.U03_E2E_PORT = port;
  return port;
}
const port = await localPort();
const desktop = resolve(import.meta.dirname, "../..");
export default defineConfig({
  ...base,
  testDir: resolve(import.meta.dirname, ".."),
  outputDir: resolve(desktop, "../../target/u03-e2e"),
  testMatch: process.env.U03_E2E_ALL === "1" ? ["**/*.spec.ts"] : ["header/*.spec.ts", "statusbar/*.spec.ts"],
  use: { ...base.use, baseURL: `http://127.0.0.1:${port}` },
  webServer: { command: `npm run dev -- --host 127.0.0.1 --port ${port}`, cwd: desktop, url: `http://127.0.0.1:${port}`, reuseExistingServer: false, timeout: 60_000 },
});
