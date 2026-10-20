import { defineConfig } from "@playwright/test";
import base from "../../playwright.config";
// Separate gate: ordinary E2E uses a bridge without the optional synthetic dataset.
export default defineConfig({ ...base, testDir: ".", testMatch: "*.gate.ts", fullyParallel: false, workers: 1,
  // Chromium headless otherwise hides native scrollbars, preventing real thumb-drag tests.
  use: { ...base.use, launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } },
  outputDir: "../../dist/grid-results", reporter: [["list"], ["json", { outputFile: "dist/grid-results/results.json" }]],
});
