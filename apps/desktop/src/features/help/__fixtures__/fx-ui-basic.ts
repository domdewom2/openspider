import type { DebugInfo } from "../../../ipc/gen/DebugInfo";
import type { ExportCatalog } from "../../../ipc/gen/ExportCatalog";
export const debugFixture: DebugInfo = {
  app: { version: "0.1.0", build: "fixture", platform: "macos", arch: "aarch64", paths: { settings: "/portable/settings", crawls: "/portable/crawls", logs: "/portable/logs", cache: "/portable/cache", exports: "/portable/exports", homeOverride: "/portable" } },
  os: "macOS fixture", pid: 42, ramLimitBytes: 2147483648, rssEngineBytes: 1000000,
  performanceProfile: "balanced", extra: [{ name: "CPU cores", value: 8 }, { name: "RAM total", value: 17179869184 }, { name: "RAM available", value: 8589934592 }, { name: "Chromium", value: "ready" }, { name: "GSC connected", value: false }],
};
export const catalogFixture: ExportCatalog = {
  bulk: [{ key: "Links", labelKey: "fixture-links", items: [{ name: "Pages With Uncrawlable Internal Outlinks", labelKey: "fixture-outlinks", enabled: true }] }],
  reports: [{ key: "JavaScript", labelKey: "fixture-js", items: [{ name: "JavaScript Console Log Summary", labelKey: "fixture-console", enabled: true }, { name: "Analysis report", labelKey: "fixture-analysis", enabled: false, disabledReason: "u04-reason-unavailable" }] }],
  sitemaps: [{ name: "pages", labelKey: "fixture-pages", enabled: true }],
};
