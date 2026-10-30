import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri erwartet einen festen Dev-Port (siehe src-tauri/tauri.conf.json). Drei Einstiegsseiten: Hauptfenster,
// Einstellungen (Fenster `settings`) und Visualisierungen (Fenster `viz-*`).
export default defineConfig({
  plugins: [react(), {
    name: "openspider-release-licenses",
    apply: "build",
    buildStart() {
      if (process.env.OPENSPIDER_REQUIRE_NOTICES !== "1") return;
      const text = readFileSync(resolve(import.meta.dirname, "src-tauri/resources/third-party/THIRD_PARTY_NOTICES.txt"), "utf8");
      if (!text.startsWith("OpenSpider — THIRD PARTY NOTICES\n") || !text.includes("\nRust: ") || !text.includes("\nnpm: ")) {
        throw new Error("Complete third-party notices required: node scripts/release/licenses/generate.mjs (repository root)");
      }
    },
  }],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        settings: resolve(import.meta.dirname, "settings.html"),
        viz: resolve(import.meta.dirname, "viz.html"),
      },
    },
  },
});
