// Additive UI-147 diagnostic, linked against the locked release artifacts.
import { readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";

const deps = "target/release/deps";
const artifacts = (name) => readdirSync(deps)
  .filter((file) => file.startsWith(`lib${name}-`) && file.endsWith(".rlib"))
  .map((file) => `${deps}/${file}`);
const native = readdirSync("target/release/build").flatMap((name) => [
  "-L", `native=target/release/build/${name}/out`,
]);
let result;
// As in U31's core runner, rustc checks feature-unified metadata variants.
for (const model of artifacts("os_model")) {
  for (const store of artifacts("os_store")) {
    result = spawnSync("rustc", [
      "--edition=2021", "-O", "-Dwarnings",
      "apps/desktop/e2e/perf/n21-profile.rs", "-o", "target/n21-profile",
      "-L", `dependency=${deps}`, ...native,
      "--extern", `os_analyze=${artifacts("os_analyze").at(-1)}`,
      "--extern", `os_model=${model}`,
      "--extern", `os_store=${store}`,
    ], { encoding: "utf8" });
    if (result.status === 0) process.exit(0);
  }
}
throw new Error(result?.stderr ?? "No matching release artifacts");
