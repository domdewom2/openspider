import { resolve } from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({ plugins: [react()], build: { target: "esnext", outDir: "dist/grid", rollupOptions: { input: resolve(import.meta.dirname, "spike.html") } } });
