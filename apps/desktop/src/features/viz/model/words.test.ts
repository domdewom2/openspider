import { describe, expect, it } from "vitest";
import { fxUiWords } from "../fixtures/synthetic";
import { fontSize, layoutWords, MAX_FONT, MIN_FONT, sortTerms } from "./words";

describe("Ankertext-Wortwolke", () => {
  // spec: UI-099
  it("Schriftgröße linear zwischen 12 px und 64 px nach Häufigkeit (fx-ui-words)", () => {
    const terms = fxUiWords();
    const counts = terms.map((t) => t.count);
    const min = Math.min(...counts);
    const max = Math.max(...counts);
    expect(fontSize(max, min, max)).toBe(MAX_FONT);
    expect(fontSize(min, min, max)).toBe(MIN_FONT);
    expect(fontSize(5, 1, 12)).toBeCloseTo(12 + (4 / 11) * 52, 9);
    expect(fontSize(3, 3, 3)).toBe(MAX_FONT);
  });

  // spec: UI-099
  it("Platzierung deterministisch bei festem Startwert; Reihenfolge nach Häufigkeit", async () => {
    const a = await layoutWords(fxUiWords(), 600, 400, "sans-serif", 42);
    const b = await layoutWords(fxUiWords(), 600, 400, "sans-serif", 42);
    expect(a).toEqual(b);
    expect(a.length).toBeGreaterThan(0);
    expect(a[0]).toMatchObject({ text: "pricing", size: MAX_FONT });
    expect(sortTerms([{ word: "b", count: 1 }, { word: "a", count: 1 }, { word: "c", count: 2 }]).map((w) => w.word)).toEqual(["c", "a", "b"]);
  });
});
