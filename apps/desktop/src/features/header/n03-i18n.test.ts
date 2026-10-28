import { expect, it } from "vitest";
import { i18n, loadFtlFiles, makeT } from "../../i18n";
import { checkCatalog } from "../../i18n/check";

// spec: UI-121
it("loads every N03 sidecar as matching German and English resources without adding a pseudo language", () => {
  const files = loadFtlFiles().filter(file => file.path.endsWith("/n03.ftl"));
  expect([...new Set(files.map(file => file.feature))].sort()).toEqual(["features/header", "features/mcp", "features/statusbar"]);
  for (const feature of new Set(files.map(file => file.feature))) {
    expect(files.filter(file => file.feature === feature).map(file => file.lang).sort()).toEqual(["de", "en"]);
  }
  expect(checkCatalog(files).problems).toEqual([]);
  expect(i18n.languages()).toEqual(["en", "de"]);
  expect(i18n.errors()).toEqual([]);
  expect(makeT("de")("n03-header-stopping")).toBe("Stoppe …");
  expect(makeT("en")("n03-header-stopping")).toBe("Stopping …");
  expect(makeT("de")("n03-status-delay-applied", { seconds: 1 })).toContain("Crawl-delay 1 s");
  expect(makeT("en")("n03-status-delay-ignored", { seconds: 1 })).toContain("not applied");
});
