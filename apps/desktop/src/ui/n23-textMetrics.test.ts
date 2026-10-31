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
  expect(requests.map(r => ellipsis(text => metrics.width(text), r.text, r.width))).toEqual(expected);
  expect(reads).toBe(preparedReads);
  metrics.setFont("20px sans-serif");
  metrics.prepare(requests);
  expect(requests.map(r => ellipsis(text => metrics.width(text), r.text, r.width))).toEqual(requests.map(r => ellipsis(text => text.length * 10, r.text, r.width)));
  expect(reads).toBeGreaterThan(preparedReads);
});
