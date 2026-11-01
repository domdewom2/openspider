import { test, expect } from "@playwright/test";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { regression } from "./report.mjs";

// spec: UI-148 [P1]
test("nightly gate: seven-run median, ten-percent boundary and missing baseline", () => {
  const previous = [900, 100, 100, 100, 100, 100, 100, 100];
  expect(regression(110, previous).status).toBe("ok");
  expect(regression(110.01, previous).status).toBe("regression");
  expect(regression(90, [100, 100, 100], "min").status).toBe("regression");
  expect(regression(100, []).status).toBe("no-baseline");
});

// spec: UI-148 [P1]
test("generator seed, real fixture size and ten-minute budget", () => {
  const result = JSON.parse(
    readFileSync(resolve("../../target/loadtest/gen-ui-large.json"), "utf8"),
  );
  expect(result.rows).toBe(Number(process.env.OS_PERF_ROWS ?? 100000));
  expect(result.seed).toBe(19);
  expect(result.elapsed_ms).toBeLessThanOrEqual(600000);
  const fixture = resolve(
    process.env.OS_PERF_FIXTURE ?? "../../target/ui-perf.ospider",
  );
  expect(statSync(fixture).size).toBe(result.file_bytes);
});
