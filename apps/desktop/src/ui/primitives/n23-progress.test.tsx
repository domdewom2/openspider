import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ProgressBar } from "./Feedback";
import tick from "../../features/header/fixtures/tick-running.json";

afterEach(cleanup);

// spec: UI-143
it("updates fixture progress through a transform while preserving the accessible range", () => {
  const view = render(<ProgressBar label="fixture crawl" value={tick.crawl.percent} />);
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", String(tick.crawl.percent));
  const fill = view.container.querySelector<HTMLElement>(".os-progress-fill")!;
  expect(fill.style.transform).toBe("scaleX(0.6)");
  expect(fill.style.width).toBe("");
  view.rerender(<ProgressBar label="fixture crawl" value={100} />);
  expect(fill.style.transform).toBe("scaleX(1)");
  view.rerender(<ProgressBar label="fixture crawl" value={null} />);
  expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
  expect(fill.style.transform).toBe("");
});
