import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { update } from "../../ipc/update";
import type { UpdateState } from "../../ipc/gen/UpdateState";
import { UpdateHost } from "./UpdateHost";
import { accept, useUpdate } from "./state";

beforeEach(() => {
  vi.useFakeTimers();
  useUpdate.setState({ state: "idle", release: null, open: false, queued: null });
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });

// spec: OUT-251
it("polls every 30 seconds at rest and every second only while checking or downloading", async () => {
  let state: UpdateState = "idle";
  const status = vi.spyOn(update, "status").mockImplementation(async () => state);
  render(<UpdateHost />);
  await act(async () => {});
  expect(status).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(29999); });
  expect(status).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(status).toHaveBeenCalledTimes(2);
  for (const next of ["checking", { downloading: { done: 5, total: 10 } }] as const) {
    state = next;
    act(() => accept(state));
    const before = status.mock.calls.length;
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(status).toHaveBeenCalledTimes(before + 1);
  }
  state = { ready: { version: "1.0.1" } };
  act(() => accept(state));
  const before = status.mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(29999); });
  expect(status).toHaveBeenCalledTimes(before);
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(status).toHaveBeenCalledTimes(before + 1);
});
