import { setTimeout as delay } from "node:timers/promises";

export function readyForKeyboard(state) {
  return state.frontend === "visible" && state.focus === true &&
    state.shell.visible === true && state.shell.minimized === false;
}

// Activation is asynchronous. A successful native request is not proof that the
// WebView owns keyboard focus. Focus the target first, then verify both layers.
export async function prepareKeyboard({ activate, snapshot, focusTarget, inspect,
  timeoutMs = 8000, intervalMs = 100, now = Date.now, sleep = delay, trace = {} }) {
  trace.native = await activate();
  await focusTarget();
  trace.samples = [];
  const deadline = now() + timeoutMs;
  let consecutive = 0;
  while (now() < deadline) {
    const state = await snapshot();
    trace.samples.push(state);
    consecutive = readyForKeyboard(state) ? consecutive + 1 : 0;
    if (consecutive >= 2) {
      trace.beforeKeyNative = await inspect();
      trace.beforeKey = await snapshot();
      if (trace.beforeKeyNative.active && readyForKeyboard(trace.beforeKey) &&
          trace.beforeKey.target === trace.target) return trace;
      // Never inject a key while another app is active. Reacquire after a
      // WebDriver action or helper invocation has disturbed focus.
      trace.native = await activate();
      await focusTarget();
      consecutive = 0;
    }
    await sleep(intervalMs);
  }
  throw new Error(`Native keyboard focus unavailable: ${JSON.stringify(trace)}`);
}
