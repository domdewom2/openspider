import assert from "node:assert/strict";

// Only observed timestamps count; a stopped crawl or hidden baseline cannot prove throttling.
export function measure(state, before, after) {
  const seconds = (after.at - before.at) / 1000;
  const ticks = after.ticks.filter(t => t.at > before.at && t.at <= after.at);
  return { name: state, status: "measured", before, after, seconds, count: ticks.length, hz: ticks.length / seconds };
}

export function validateRate(measurement) {
  const { name, before, after, seconds, count, hz } = measurement;
  assert.ok(seconds >= 30, "Each state requires at least 30 seconds");
  const ticks = after.ticks.filter(t => t.at > before.at && t.at <= after.at);
  assert.ok(count > 0 && ticks.every(t => t.phase === "running"), "Crawl must remain active throughout measurement");
  const hidden = ["minimized", "covered"].includes(name);
  for (const snapshot of [before, after]) {
    assert.equal(snapshot.frontend, hidden ? "hidden" : "visible", `${name}: unexpected frontend visibility`);
    assert.equal(snapshot.shell.minimized, name === "minimized", `${name}: unexpected minimization`);
  }
  if (hidden) assert.ok(count <= Math.floor(seconds) + 1, `hidden rate ${hz} Hz > 1 Hz (+one boundary tick)`);
  else assert.ok(hz > 1, `visible rate ${hz} Hz did not recover`);
}
