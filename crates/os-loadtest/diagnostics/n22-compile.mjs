// Additive standalone diagnostics; the U31 harness and budgets remain untouched.
import { readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
const deps = "target/release/deps";
const artifacts = (name) => readdirSync(deps)
  .filter((n) => n.startsWith(`lib${name}-`) && n.endsWith(".rlib"))
  .map((n) => `${deps}/${n}`);
const native = readdirSync("target/release/build").flatMap((n) => [
  "-L", `native=target/release/build/${n}/out`,
]);
const source = process.argv[2] ?? "crates/os-loadtest/diagnostics/n22-core.rs";
const output = process.argv[3] ?? "target/n22-profile";
let last;
for (const analyze of artifacts("os_analyze")) {
  for (const model of artifacts("os_model")) {
    for (const store of artifacts("os_store")) {
      const result = spawnSync("rustc", [
        "--edition=2021", "-O", "-Dwarnings", source, "-o", output,
        "-L", `dependency=${deps}`, ...native,
        "--extern", `os_analyze=${analyze}`,
        "--extern", `os_model=${model}`,
        "--extern", `os_store=${store}`,
      ], { encoding: "utf8" });
      if (result.status === 0) process.exit(0);
      last = result.stderr;
    }
  }
}
throw new Error(last ?? "Missing locked release artifacts");
