import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ComboBox } from "./ComboBox";

afterEach(cleanup);
const options = [
  { id: "all", label: "All", count: 3 },
  { id: "blocked", label: "Unavailable", isDisabled: true, hint: "Requires analysis" },
  { id: "html", label: "HTML", count: 2 },
];
function Fixture() {
  const [selected, setSelected] = useState("all");
  return <><button>Before</button><ComboBox label="Filter" options={options} selected={selected} onSelect={setSelected} /><button>After</button></>;
}

// spec: UI-126
it("keeps surrounding controls exposed, selects with the keyboard and skips disabled options", async () => {
  render(<Fixture />);
  const input = screen.getByRole("combobox", { name: "Filter" });
  fireEvent.focus(input);
  fireEvent.keyDown(input, { key: "ArrowDown" });
  await screen.findByRole("listbox", { name: "Filter" });
  expect(screen.getByRole("button", { name: "Before" }).closest('[aria-hidden="true"]')).toBeNull();
  expect(screen.getByRole("button", { name: "After" }).closest('[aria-hidden="true"]')).toBeNull();
  fireEvent.keyDown(input, { key: "ArrowDown" });
  const active = document.getElementById(input.getAttribute("aria-activedescendant")!);
  expect(active).toHaveTextContent("HTML");
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  expect(input).toHaveValue("HTML (2)");
});

// spec: UI-126
it("filters by typing, restores the selection on Escape, and lets Tab leave the popup", async () => {
  render(<Fixture />);
  const input = screen.getByRole("combobox", { name: "Filter" });
  fireEvent.change(input, { target: { value: "ht" } });
  expect(await screen.findAllByRole("option")).toHaveLength(1);
  fireEvent.keyDown(input, { key: "Escape" });
  expect(input).toHaveValue("All (3)");
  fireEvent.keyDown(input, { key: "ArrowDown" });
  await screen.findByRole("listbox");
  expect(fireEvent.keyDown(input, { key: "Tab" })).toBe(true);
  await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  expect(input).toHaveAttribute("aria-expanded", "false");
});
