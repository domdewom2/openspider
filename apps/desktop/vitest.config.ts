import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      setupFiles: ["./src/test-setup.ts"],
      globals: false,
      include: ["src/**/*.test.{ts,tsx}"],
      exclude: ["e2e/**", "node_modules/**"],
    },
  }),
);
