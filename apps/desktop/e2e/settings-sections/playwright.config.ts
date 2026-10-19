import { createServer } from "node:net";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";
import base from "../../playwright.config";

// Keep existing desktop dev servers untouched; each run gets an ephemeral loopback port.
async function testPort(): Promise<number> {
  if (process.env.OS_SETTINGS_E2E_PORT) return Number(process.env.OS_SETTINGS_E2E_PORT);
  const server = createServer();
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("No test port");
  const port = address.port;
  await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  process.env.OS_SETTINGS_E2E_PORT = String(port);
  return port;
}
const port = await testPort();
const baseURL = `http://127.0.0.1:${port}`;
export default defineConfig({
  ...base,
  testDir: ".",
  outputDir: "../../test-results/settings-sections",
  use: { ...base.use, baseURL },
  webServer: {
    command: `npm run dev -- --host 127.0.0.1 --port ${port}`,
    cwd: fileURLToPath(new URL("../..", import.meta.url)),
    url: baseURL,
    reuseExistingServer: false,
    timeout: 60_000,
    stdout: "ignore",
  },
});
