import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { stripVTControlCharacters } from "node:util";
import test from "node:test";
import { viteReady } from "./readiness.mjs";

// spec: UI-002, UI-113 — native fixture startup must work with CI terminal colors.
test("recognizes all five recorded CI startup logs, colored and plain", async () => {
  for (const artifact of [
    "s02-smoke-windows-latest", "s02-smoke-macos-15",
    "s02-s3-windows-latest", "s02-s3-macos-15", "s02-s6-macos-15",
  ]) {
    const log = await readFile(new URL(`../spikes/evidence/review5-ci/${artifact}/native.log`, import.meta.url), "utf8");
    assert.equal(viteReady(log), true, artifact);
    assert.equal(viteReady(stripVTControlCharacters(log)), true, artifact);
  }
});

// spec: UI-002, UI-113
test("waits for the complete endpoint across output chunks", () => {
  let output = "\u001b[1mLocal\u001b[22m: http://localhost:\u001b[1m14";
  assert.equal(viteReady(output), false);
  output += "20\u001b[22m/\u001b[39m\n";
  assert.equal(viteReady(output), true);
});

// spec: UI-002, UI-113
test("rejects missing readiness and a different dev server port", () => {
  for (const output of ["", "VITE ready in 245 ms", "Error: Port 1420 is already in use",
    "Local: http://localhost:1421/", "Local: http://localhost:14200/",
    "Network: http://localhost:1420/"]) {
    assert.equal(viteReady(output), false, output);
  }
});
