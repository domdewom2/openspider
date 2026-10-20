import assert from "node:assert/strict";
import test from "node:test";
import { measure, validateRate } from "./measurement.mjs";

function fixture(state, count, seconds = 30) {
  const snapshot = { frontend: ["minimized", "covered"].includes(state) ? "hidden" : "visible",
    shell: { minimized: state === "minimized" } };
  const before = { ...snapshot, at: 1000, ticks: [] };
  const after = { ...snapshot, at: 1000 + seconds * 1000,
    ticks: Array.from({ length: count }, (_, i) => ({ at: 1000 + (i + 1) * seconds * 1000 / count, phase: "running" })) };
  return measure(state, before, after);
}

// spec: UI-143
test("five-state fixture accepts throttling and visible recovery with a boundary tick", () => {
  for (const state of ["visible", "minimized", "restored", "covered", "uncovered"]) {
    const value = fixture(state, ["minimized", "covered"].includes(state) ? 31 : 120);
    validateRate(value);
    assert.equal(value.status, "measured");
    assert.equal(value.seconds, 30);
  }
});

// spec: UI-143
test("empty, stopped, too short and over-limit observations never pass", () => {
  assert.throws(() => validateRate(fixture("minimized", 0)), /remain active/);
  const stopped = fixture("minimized", 30);
  stopped.after.ticks[20].phase = "completed";
  assert.throws(() => validateRate(stopped), /remain active/);
  assert.throws(() => validateRate(fixture("covered", 29, 29)), /30 seconds/);
  assert.throws(() => validateRate(fixture("covered", 32)), /hidden rate/);
  assert.throws(() => validateRate(fixture("restored", 30)), /did not recover/);
});

// spec: UI-143
test("locked or unrestored windows cannot be accepted as a visible baseline", () => {
  const locked = fixture("visible", 120);
  locked.before.frontend = "hidden";
  assert.throws(() => validateRate(locked), /visibility/);
  const unrestored = fixture("restored", 120);
  unrestored.after.shell.minimized = true;
  assert.throws(() => validateRate(unrestored), /minimization/);
});

// spec: UI-143
test("rate excludes ticks outside the measured interval", () => {
  const value = fixture("covered", 30);
  value.after.ticks.push({ at: value.before.at, phase: "running" }, { at: value.after.at + 1, phase: "running" });
  const measured = measure("covered", value.before, value.after);
  assert.equal(measured.count, 30);
  assert.equal(measured.hz, 1);
  validateRate(measured);
});
