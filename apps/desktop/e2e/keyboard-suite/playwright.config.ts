import { defineConfig } from "@playwright/test";
import base from "../visual/playwright.config";
export default defineConfig({ ...base, testDir: ".", outputDir: "../../dist/u32-keyboard", reporter: [["list"], ["json", { outputFile: "../../dist/u32-keyboard/results.json" }]], projects: [
  { name: "chromium", use: { browserName: "chromium" } }, { name: "webkit", use: { browserName: "webkit" } },
] });
