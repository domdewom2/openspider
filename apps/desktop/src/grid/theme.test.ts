import { rowWindow, scrollExtent } from "./geometry";
import { expect, it } from "vitest";
import { cellBadge, gridHeaderHeight } from "./theme";
import { THEME_TOKENS } from "../ui/tokens/tokens";
import { contrast } from "../ui/tokens/color";
import fixture from "./fixtures/fx-ui-basic.json";
// spec: UI-120
it("K2-77 maps both densities to the specified heads without changing arbitrary spike heights", () => {
  expect(gridHeaderHeight(28)).toBe(32);
  expect(gridHeaderHeight(44)).toBe(44);
  expect(gridHeaderHeight(26)).toBe(30);
});
// spec: UI-126
it("badge text remains readable for the fixture statuses in both themes and data cells stay plain", () => {
  for (const tokens of Object.values(THEME_TOKENS)) {
    for (const [key, text] of [["Status Code", "200"], ["Status Code", "301"], ["Status Code", "404"], ["Status Code", "503"], ["Status Code", "0"], ["Status", "OK"], ["Indexability", "Indexable"], ["Indexability", "Non-Indexable"]]) {
      const badge = cellBadge(key, text, tokens)!;
      expect(contrast(badge.text, badge.background)).toBeGreaterThanOrEqual(4.5);
    }
    expect(cellBadge("Address", String(fixture.row[1]), tokens)).toBeNull();
    expect(cellBadge("Status Code", "…", tokens)).toBeNull();
  }
});

// spec: UI-120
it("fills the final comfortable viewport with eight complete fixture rows below the 44px head", () => {
  const last = rowWindow(fixture.total, 396, 44);
  expect(fixture.total - last.maxFirst).toBe(8);
  expect(last.capacity).toBe(8);
  expect(scrollExtent(25, 396, 44)).toBe(1144);
});
