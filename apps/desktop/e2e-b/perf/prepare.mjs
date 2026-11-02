import { resolve } from "node:path";
import { writeFileSync } from "node:fs";
// Override build/entry only; product sources and normal Tauri configuration remain untouched.
writeFileSync(
  "target/ui-perf-native.conf.json",
  JSON.stringify(
    {
      build: {
        beforeBuildCommand:
          "npm exec -- vite build --config e2e/perf/vite.config.ts",
        frontendDist: resolve("target/ui-perf-build"),
      },
      app: {
        windows: [
          {
            label: "main",
            title: "OpenSpider",
            url: "e2e/perf/shell.html",
            width: 1280,
            height: 800,
          },
        ],
      },
      bundle: { active: false },
    },
    null,
    2,
  ),
);
