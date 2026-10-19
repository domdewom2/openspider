/** S-1 recorder: retain raw intervals as evidence, never infer native results from Playwright. */
export async function recordFrames(step: (elapsed: number) => void, durationMs = 10_000) {
  const frames: number[] = [];
  const longFrames: number[] = [];
  const observer = typeof PerformanceObserver !== "undefined" && PerformanceObserver.supportedEntryTypes.includes("long-animation-frame")
    ? new PerformanceObserver(list => longFrames.push(...list.getEntries().map(e => e.duration))) : undefined;
  observer?.observe({ type: "long-animation-frame", buffered: false });
  await new Promise<void>(resolve => {
    let start: number | undefined; let previous: number;
    const tick = (at: number) => {
      start ??= at;
      if (previous !== undefined) frames.push(at - previous);
      previous = at;
      step(at - start);
      if (at - start < durationMs) requestAnimationFrame(tick); else resolve();
    };
    requestAnimationFrame(tick);
  });
  observer?.disconnect();
  const elapsed = frames.reduce((sum, value) => sum + value, 0);
  const ordered = [...frames].sort((a, b) => a - b);
  return { durationMs: elapsed, frames, longFrames, fps: frames.length / elapsed * 1000,
    maxFrameMs: Math.max(...frames), p95FrameMs: ordered[Math.floor(ordered.length * 0.95)] };
}
export const paint = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
