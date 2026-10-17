import { resolve } from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri erwartet einen festen Dev-Port (siehe src-tauri/tauri.conf.json). Drei Einstiegsseiten: Hauptfenster,
// Einstellungen (Fenster `settings`) und Visualisierungen (Fenster `viz-*`).
export default defineConfig({
  plugins: [react()],
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
