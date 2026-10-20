import assert from "node:assert/strict";
import test from "node:test";
import { prepareKeyboard } from "./focus.mjs";

const visible = { frontend: "visible", focus: true, shell: { visible: true, minimized: false }, target: "s02-input" };
const hidden = { ...visible, frontend: "hidden", focus: false };

function fixture(states, nativeStates = [true]) {
  let clock = 0, index = 0, nativeIndex = 0;
  const calls = [];
  return {
    calls, trace: { target: "s02-input" },
    activate: async () => { calls.push("activate"); return { requested: true }; },
    snapshot: async () => { calls.push("snapshot"); return states[Math.min(index++, states.length - 1)]; },
    focusTarget: async () => { calls.push("focusTarget"); },
    inspect: async () => { calls.push("inspect"); return { active: nativeStates[Math.min(nativeIndex++, nativeStates.length - 1)] }; },
    now: () => clock, sleep: async ms => { clock += ms; }, timeoutMs: 1000, intervalMs: 100,
  };
}

// spec: UI-002, UI-113 — asynchronous activation must settle before native input.
test("waits for visible WebView focus, then verifies focus after targeting", async () => {
  const f = fixture([hidden, { ...visible, focus: false }, visible, visible, visible]);
  const result = await prepareKeyboard(f);
  assert.equal(result.samples.length, 4);
  assert.deepEqual(f.calls.slice(0, 3), ["activate", "focusTarget", "snapshot"]);
  assert.deepEqual(f.calls.slice(-2), ["inspect", "snapshot"]);
  assert.equal(result.beforeKey.focus, true);
});

// spec: UI-113 — helpers/WebDriver must not leave keyboard input in another app.
test("reacquires focus when the native foreground changes after targeting", async () => {
  const f = fixture([visible], [false, true]);
  await prepareKeyboard(f);
  assert.equal(f.calls.filter(c => c === "activate").length, 2);
  assert.equal(f.calls.filter(c => c === "focusTarget").length, 2);
});

// spec: UI-113 — transient focus must not satisfy the settling requirement.
test("resets stability on focus loss and rechecks the DOM after targeting", async () => {
  const f = fixture([visible, hidden, visible, visible, hidden, visible, visible, visible]);
  const result = await prepareKeyboard(f);
  assert.equal(result.samples.length, 6);
  assert.equal(f.calls.filter(c => c === "activate").length, 2);
  assert.equal(result.beforeKey.focus, true);
});

// spec: UI-113 — visibility alone or a native activation return value is insufficient.
test("fails closed for hidden, unfocused, minimized, or wrong-target fixtures", async () => {
  for (const state of [hidden, { ...visible, focus: false },
    { ...visible, shell: { visible: false, minimized: false } },
    { ...visible, shell: { visible: true, minimized: true } },
    { ...visible, target: "another-input" }]) {
    const f = fixture([state]);
    await assert.rejects(prepareKeyboard(f), /Native keyboard focus unavailable/);
    assert.ok(f.trace.samples.length);
  }
});
