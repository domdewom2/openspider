import { afterEach, expect, it, vi } from "vitest";
import { DisplayTextMetrics } from "../grid/textMetrics";
import { ellipsis } from "../grid/cells";
import fixture from "../grid/fixtures/fx-ui-basic.json";

afterEach(() => { vi.restoreAllMocks(); document.body.replaceChildren(); });

// spec: UI-140
// spec: UI-143
it("batched fixture text clipping preserves widths and Unicode boundaries, and invalidates on font change", () => {
  const element = document.createElement("span");
  document.body.appendChild(element);
  let reads = 0;
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function(this: HTMLElement) {
    reads++;
    return (this.textContent ?? "").length * (element.style.font.startsWith("20px") ? 10 : 7);
  });
  const metrics = new DisplayTextMetrics(element);
  metrics.setFont("12px sans-serif");
  const requests = [...fixture.row.map(value => String(value ?? "")), "A😀B", "https://fixture.invalid/long/path"].map(text => ({ text, width: 24 }));
  const expected = requests.map(r => ellipsis(text => text.length * 7, r.text, r.width));
  metrics.prepare(requests);
  const preparedReads = reads;
  // Same flow as the grid's draw loop: a text that fits by its upper bound is drawn unchanged.
  expect(requests.map(r => metrics.fits(r.text, r.width) ? r.text : ellipsis(text => metrics.width(text), r.text, r.width))).toEqual(expected);
  expect(reads).toBe(preparedReads);
  metrics.setFont("20px sans-serif");
  metrics.prepare(requests);
  expect(requests.map(r => ellipsis(text => metrics.width(text), r.text, r.width))).toEqual(requests.map(r => ellipsis(text => text.length * 10, r.text, r.width)));
  expect(reads).toBeGreaterThan(preparedReads);
});

// spec: UI-144
// spec: UI-140
it("does not lay out whole fixture texts whose per-character upper bound fits, with unchanged clipping", () => {
  const element = document.createElement("span");
  document.body.appendChild(element);
  const laidOut: string[] = [];
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function(this: HTMLElement) {
    laidOut.push(this.textContent ?? "");
    return (this.textContent ?? "").length * 7;
  });
  const metrics = new DisplayTextMetrics(element);
  metrics.setFont("12px sans-serif");
  const values = [...fixture.row.map(value => String(value ?? "")), "A😀B", "https://fixture.invalid/long/path"];
  const requests = [...values.flatMap(text => [24, 84, 400].map(width => ({ text, width }))),
    ...["12,345", "9.876"].flatMap(text => [84, 400].map(width => ({ text, width })))];
  metrics.prepare(requests);
  const exact = (text: string) => text.length * 7;
  for (const r of requests) {
    const shown = metrics.fits(r.text, r.width) ? r.text : ellipsis(text => metrics.width(text), r.text, r.width);
    expect(shown).toBe(ellipsis(exact, r.text, r.width));
    // An upper bound never claims a fit that exact measurement would reject.
    if (metrics.fits(r.text, r.width)) expect(exact(r.text)).toBeLessThanOrEqual(r.width);
  }
  // Short numbers fit their column by the bound and were never laid out as whole strings; only single characters were.
  expect(laidOut).not.toContain("12,345");
  expect(laidOut).not.toContain("9.876");
  expect(laidOut.filter(text => text.length > 1).every(text => values.includes(text) || text.endsWith("…"))).toBe(true);
  // Non-ASCII text has no bound and is measured as before.
  expect(metrics.fits("A😀B", 400)).toBe(true);
  expect(laidOut).toContain("A😀B");
  // A font change drops the per-character advances.
  metrics.setFont("20px sans-serif");
  expect(metrics.fits("12,345", 400)).toBe(false);
});
