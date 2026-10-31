import { defineConfig, devices } from "@playwright/test";

// E2E-A „Frontend + Kern“ (M2-ui-concept 7.3): Playwright gegen das Vite-Frontend, IPC über die E2E-Bridge `os-app-bridge`
// (Umgebung OS_E2E_TRANSPORT=bridge|mock|auto, siehe e2e/support/bridge.ts). Engines: WebKit (≈ WKWebView) und Chromium
// (≈ WebView2); auf macOS-CI nur WebKit, auf Windows-CI nur Chromium (OS_E2E_PROJECTS), in der Cloud (Linux) beide.
const wanted = (process.env.OS_E2E_PROJECTS ?? "chromium,webkit").split(",").map((s) => s.trim());

// Laufzeitbudgets (UI-146) werden erst nach allen übrigen Tests und ohne parallele Last gemessen: je Engine ein eigenes
// Projekt `<engine>-perf`, das vom Hauptprojekt abhängt. `OS_E2E_PROJECTS` nennt weiterhin nur die Engines.
const PERF = /viz[\\/]perf\.spec\.ts$/;
// Visual snapshots require the dedicated platform/scale configuration.
const MAIN_IGNORE = [PERF, /visual[\\/]/];
const engines = [
  { name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
  { name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1280, height: 800 } } },
];
const all = engines.flatMap((engine) => [
  { ...engine, testIgnore: MAIN_IGNORE },
  { name: `${engine.name}-perf`, use: engine.use, testMatch: PERF, dependencies: [engine.name] },
]);

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: "http://localhost:1420",
    trace: "retain-on-failure",
    reducedMotion: "reduce",
  },
  projects: all.filter((p) => wanted.includes(p.name.replace(/-perf$/, ""))),
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    stdout: "ignore",
  },
});
