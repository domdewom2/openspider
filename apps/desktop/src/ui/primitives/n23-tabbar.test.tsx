import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TabBar } from "./TabBar";
import { usePrefs } from "../../state/prefs";
import fixture from "../../grid/fixtures/fx-ui-basic.json";

afterEach(cleanup);

// spec: UI-143
it("does not remeasure equal fixture labels on a live tick; language, selection and appearance still remeasure", () => {
  const tabs = fixture.columns.map((c, i) => ({ id: String(i), label: c.key }));
  const measure = vi.fn(() => ({ widths: tabs.map(() => 80), available: 2000, overflowWidth: 32 }));
  const props = { selected: "0", onSelect: () => {}, label: "fixture", overflowLabel: "More", measure };
  const view = render(<TabBar {...props} tabs={tabs} />);
  const initial = measure.mock.calls.length;
  view.rerender(<TabBar {...props} tabs={tabs.map(t => ({ ...t }))} />);
  expect(measure).toHaveBeenCalledTimes(initial);
  view.rerender(<TabBar {...props} tabs={tabs.map(t => ({ ...t, label: `${t.label} de` }))} />);
  expect(measure.mock.calls.length).toBeGreaterThan(initial);
  const renamed = measure.mock.calls.length;
  view.rerender(<TabBar {...props} selected="1" tabs={tabs.map(t => ({ ...t, label: `${t.label} de` }))} />);
  expect(measure.mock.calls.length).toBeGreaterThan(renamed);
  const selected = measure.mock.calls.length;
  const before = usePrefs.getState().appearance;
  act(() => usePrefs.setState({ appearance: { ...before, zoomPct: before.zoomPct === 100 ? 125 : 100 } }));
  expect(measure.mock.calls.length).toBeGreaterThan(selected);
  view.unmount();
  usePrefs.setState({ appearance: before });
});
