import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist/mcp-native",
    emptyOutDir: true,
    rollupOptions: { input: fileURLToPath(new URL("./native.html", import.meta.url)) },
  },
});
