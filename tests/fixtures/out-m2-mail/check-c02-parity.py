#!/usr/bin/env python3
"""Probe OUT-282 against a local C02 commit without merging or editing its worktree.

Requires Python 3.9+ and the normal Rust test environment. Only C02's public
analysis module is overlaid into a disposable T02 checkout. This probe does not
replace the post-merge command/lifecycle tests or the platform CI gate.
"""

import argparse
import io
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[3]
CHANGE = "crates/os-analyze/src/compare/change/"
TEST = "crates/os-app/tests/cmd_mail_lifecycle.rs"
MARKER = "    // C02's manual Change Detection interface is still a stub on this branch."

# spec: OUT-282
PROBE = """
    // Use exactly the API used by C02's manual comparison, on the same files.
    let cfg = &cmp.current().config.compare.change_detection;
    os_analyze::compare::change::calculate(&cmp, cfg, &cancel).unwrap();
    let counts = os_analyze::compare::change::counts(&cmp, cfg, false).unwrap();
    assert_eq!(counts.len(), 14);
    assert_eq!(changes.len(), manual.len() + 2 + counts.len());
    let mut differences = Vec::new();
    for row in counts {
        let ids = os_analyze::compare::change::ids(&cmp, &row.element, true).unwrap();
        assert_eq!(row.current, ids.len(), "C02 public count vs IDs");
        let mail = changes[manual.len() + 2..].iter()
            .find(|r| r["name"] == row.element).expect("mail element row");
        if mail["current"] != row.current {
            differences.push(format!("{}: mail={}, C02={}, current IDs={:?}",
                row.element, mail["current"], row.current, ids.iter().collect::<Vec<_>>()));
        }
    }
    assert!(differences.is_empty(), "OUT-282 parity failed:\\n{}", differences.join("\\n"));
"""


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--c02-ref", default="m2/c02", help="Local C02 commit/ref; never fetched")
    args = parser.parse_args()
    c02 = git("rev-parse", "--verify", f"{args.c02_ref}^{{commit}}").decode().strip()
    t02 = git("rev-parse", "HEAD").decode().strip()
    print(f"T02 base={t02}; C02 analysis={c02}; T02 test from current worktree", flush=True)
    files = git("ls-tree", "-r", "--name-only", c02, "--", CHANGE).decode().splitlines()
    if not files or CHANGE + "mod.rs" not in files:
        parser.error("C02 ref has no public compare::change implementation")
    with tempfile.TemporaryDirectory(prefix="openspider-t02-c02-") as tmp:
        scratch = Path(tmp)
        with zipfile.ZipFile(io.BytesIO(git("archive", "--format=zip", "HEAD"))) as archive:
            archive.extractall(scratch)
        for name in files:
            target = scratch / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(git("show", f"{c02}:{name}"))
        module = scratch / "crates/os-analyze/src/compare/mod.rs"
        source = module.read_text()
        if "pub mod change;" not in source:
            module.write_text(source.replace("pub mod classify;", "pub mod change;\npub mod classify;", 1))
        source = (ROOT / TEST).read_text()
        if source.count(MARKER) != 1:
            parser.error("T02 test marker changed; update the probe before running")
        (scratch / TEST).write_text(source.replace(MARKER, PROBE + "\n" + MARKER, 1))
        env = os.environ.copy()
        # Keep all artifacts in the caller's package target, never in C02's target.
        env["CARGO_TARGET_DIR"] = str(Path(env.get("CARGO_TARGET_DIR", ROOT / "target")).resolve())
        return subprocess.run(
            ["cargo", "test", "--locked", "-p", "os-app", "--test", "cmd_mail_lifecycle",
             "overview_and_all_comparison_rows_match_manual_queries_and_fixture_golden", "--", "--nocapture"],
            cwd=scratch, env=env, check=False,
        ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
