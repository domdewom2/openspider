import { createServer } from "node:net";
import { defineConfig } from "@playwright/test";
import base from "../../playwright.config";
// Package-local server avoids reusing another worktree's Vite instance on the shared macOS host.
const port = process.env.OS_U10_E2E_PORT ?? await new Promise<string>((resolve, reject) => {
  const socket = createServer(); socket.once("error", reject);
  socket.listen(0, "127.0.0.1", () => {
    const address = socket.address();
    if (!address || typeof address === "string") { reject(new Error("No ephemeral port")); return; }
    socket.close(() => resolve(String(address.port)));
  });
});
process.env.OS_U10_E2E_PORT = port;
export default defineConfig({
  ...base,
  testDir: ".",
  testMatch: "*.spec.ts",
  outputDir: ".artifacts",
  use: { ...base.use, baseURL: `http://127.0.0.1:${port}` },
  webServer: { command: `npm run dev -- --host 127.0.0.1 --port ${port}`, url: `http://127.0.0.1:${port}`, reuseExistingServer: false, timeout: 60_000 },
});
