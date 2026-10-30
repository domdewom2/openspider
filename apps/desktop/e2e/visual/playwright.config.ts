import { execFileSync } from "node:child_process";
import { defineConfig } from "@playwright/test";

const port = Number(process.env.OS_U32_PORT ??= execFileSync(process.execPath, ["-e", "const s=require('node:net').createServer();s.listen(0,'127.0.0.1',()=>{process.stdout.write(String(s.address().port));s.close()})"], { encoding: "utf8" }));
const baseURL = `http://127.0.0.1:${port}`;
export default defineConfig({
  testDir: ".", testMatch: "*.spec.ts", fullyParallel: true, workers: 4,
  timeout: 60_000, expect: { timeout: 15_000 }, retries: 0,
  snapshotPathTemplate: "{testDir}/baselines/{platform}/{projectName}/{testFilePath}/{arg}{ext}",
  outputDir: "../../dist/u32-visual", reporter: [["list"], ["json", { outputFile: "../../dist/u32-visual/results.json" }]],
  use: { baseURL, reducedMotion: "reduce", trace: "retain-on-failure" },
  projects: [
    ...[1, 1.25, 1.5, 2].map((deviceScaleFactor) => ({ name: `chromium-${deviceScaleFactor}`, ...(deviceScaleFactor !== 1 ? { testIgnore: ["screenshots.spec.ts", "change-detection.spec.ts"] } : {}), use: { browserName: "chromium" as const, deviceScaleFactor } })),
    { name: "webkit-2", use: { browserName: "webkit", deviceScaleFactor: 2 } },
  ],
  webServer: { command: `npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`, url: baseURL, reuseExistingServer: false, timeout: 60_000 },
});
