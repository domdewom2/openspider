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
      // Langsame CI-Runner (macOS Intel, Windows): großzügige Grenzen statt Flakes.
      testTimeout: 30_000,
      hookTimeout: 30_000,
    },
  }),
);
