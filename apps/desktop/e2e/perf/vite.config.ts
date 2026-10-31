import { resolve } from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  define: { "import.meta.env.VITE_E2E": JSON.stringify("1") },
  build: {
    target: "esnext",
    outDir: "../../target/ui-perf-build",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        perf: resolve(import.meta.dirname, "index.html"),
        load: resolve(import.meta.dirname, "shell.html"),
        main: resolve("index.html"),
        viz: resolve("viz.html"),
      },
    },
  },
});
