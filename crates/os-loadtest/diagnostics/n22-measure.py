"""Additive Unix coordinator; invokes the unchanged U31 binary, never edits its verdict.

Start below load 2; retain every attempt and accept a measurement only when its
one-minute load stayed below 4. Budget failures remain failures, without retries.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def measure(phase, rows, output):
    fixture = Path(f"target/n22-{rows}.ospider")
    metadata = json.loads(Path(f"{fixture}.json").read_text())
    assert metadata["rows"] == rows
    seed = 19 if rows == 5_000_000 else 20
    assert metadata["seed"] == seed
    binary = Path(f"target/n22-core-{phase}").resolve()
    attempt = 1
    while True:
        folder = output / f"attempt-{attempt}"
        if (folder / f"{phase}-{rows}.load.json").exists():
            attempt += 1
            continue
        print(f"Waiting for load <2: {phase} {rows} attempt {attempt}", flush=True)
        while os.getloadavg()[0] >= 2:
            time.sleep(10)
        folder.mkdir(parents=True, exist_ok=True)
        stem = f"{phase}-{rows}"
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        command = [str(binary), str(fixture), str(rows), started, str(seed)]
        timer = shutil.which("time")
        if timer and sys.platform == "linux":
            command = [timer, "-v", "-o", str(folder / f"{stem}.memory.txt"), *command]
        samples = [(0.0, os.getloadavg())]
        at = time.monotonic()
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        while process.poll() is None:
            time.sleep(0.25)
            load = os.getloadavg()
            if load != samples[-1][1]:
                samples.append((round(time.monotonic() - at, 3), load))
            if time.monotonic() - at > 600:
                process.kill()
                break
        stdout, stderr = process.communicate()
        samples.append((round(time.monotonic() - at, 3), os.getloadavg()))
        peak = max(load[0] for _, load in samples)
        valid = peak < 4
        (folder / f"{stem}.stdout").write_text(stdout)
        (folder / f"{stem}.stderr").write_text(stderr)
        (folder / f"{stem}.load.json").write_text(json.dumps({
            "started": started, "load_before": samples[0][1],
            "load_after": samples[-1][1], "load_samples": samples,
            "peak_load_1m": peak, "valid_quiet": valid, "exit_status": process.returncode,
            "binary": str(binary.relative_to(Path.cwd())),
        }, indent=2) + "\n")
        if process.returncode:
            raise RuntimeError(f"Core runner exited {process.returncode}: {stderr}")
        report = json.loads(stdout)
        (folder / f"{stem}.json").write_text(json.dumps(report, indent=2) + "\n")
        print(stem, f"peak load={peak:.2f}",
              {k: round(v["value"], 3) for k, v in report["metrics"].items()}, flush=True)
        if valid:
            for path in folder.glob(f"{stem}.*"):
                shutil.copyfile(path, output / path.name)
            return
        print("Load crossed 4; retain attempt as diagnostic and wait again.", flush=True)
        attempt += 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=["before", "after"])
    parser.add_argument("--rows", type=int, choices=[1_000_000, 5_000_000])
    parser.add_argument("--output", type=Path, default=Path("target/n22-results"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for rows in ([args.rows] if args.rows else [1_000_000, 5_000_000]):
        for phase in ([args.phase] if args.phase else ["before", "after"]):
            measure(phase, rows, args.output)


if __name__ == "__main__":
    main()
